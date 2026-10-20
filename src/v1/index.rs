use super::state;
use crate::{
    advanced, documents,
    library::{self, SharedFolder},
    writing,
};
use rmcp::schemars;
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs,
    time::Duration,
};
#[derive(Deserialize, schemars::JsonSchema)]
pub struct IndexArgs {
    #[serde(default)]
    pub folder: String,
    pub offset: Option<usize>,
    pub max_files: Option<usize>,
}
#[derive(Deserialize, schemars::JsonSchema)]
pub struct QueryArgs {
    #[serde(default)]
    pub folder: String,
    pub query: String,
    pub limit: Option<usize>,
}
fn database(folders: &[SharedFolder], folder: &str) -> Result<Connection, String> {
    let shared = library::select_folder(folders, folder)?;
    let path = state::root()?.join(format!(
        "index-{}.sqlite",
        state::scope(&shared.path.canonicalize().map_err(|e| e.to_string())?)
    ));
    let db = Connection::open(path).map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA trusted_schema=OFF;PRAGMA max_page_count=32768;CREATE TABLE IF NOT EXISTS documents(path TEXT PRIMARY KEY,sha TEXT NOT NULL,updated INTEGER);CREATE VIRTUAL TABLE IF NOT EXISTS search USING fts5(path UNINDEXED,page UNINDEXED,sheet UNINDEXED,sha UNINDEXED,text,tokenize='unicode61 remove_diacritics 2');").map_err(|e|e.to_string())?;
    Ok(db)
}
pub fn cached(hash: &str) -> Option<Vec<documents::Unit>> {
    let path = state::root()
        .ok()?
        .join("units")
        .join(format!("{hash}.json"));
    if fs::metadata(&path).ok()?.len() > 20 * 1024 * 1024 {
        return None;
    }
    let data: Vec<documents::Unit> = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    if data.len() > 500 || data.iter().map(|u| u.text.len()).sum::<usize>() > 16 * 1024 * 1024 {
        return None;
    }
    Some(data)
}
pub fn cache(hash: &str, units: &[documents::Unit]) -> Result<(), String> {
    let _lock = state::LOCK.lock().map_err(|_| "Cache occupé")?;
    let dir = state::root()?.join("units");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(units).map_err(|e| e.to_string())?;
    let mut files: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_file()
                .then_some((m.modified().ok(), m.len(), e.path()))
        })
        .collect();
    files.sort_by_key(|f| f.0);
    let mut total = files.iter().map(|f| f.1).sum::<u64>();
    let mut count = files.len();
    for (_, size, path) in files {
        if total + bytes.len() as u64 <= 128 * 1024 * 1024 && count < 1024 {
            break;
        }
        fs::remove_file(path).map_err(|e| e.to_string())?;
        total = total.saturating_sub(size);
        count -= 1;
    }
    state::write_json(&dir.join(format!("{hash}.json")), &units)
}
pub fn build(folders: &[SharedFolder], args: IndexArgs) -> Result<Value, String> {
    let (files, truncated) = documents::paths(folders, &args.folder, "", true, None)?;
    let files: Vec<_> = files
        .into_iter()
        .filter(|(_, d)| !*d)
        .map(|(p, _)| p)
        .collect();
    let mut db = database(folders, &args.folder)?;
    let existing: Vec<String> = db
        .prepare("SELECT path FROM documents")
        .map_err(|e| e.to_string())?
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    let allowed: HashSet<_> = files.iter().collect();
    for path in existing {
        if !allowed.contains(&path) {
            let tx = db.transaction().map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM documents WHERE path=?", [&path])
                .map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM search WHERE path=?", [&path])
                .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
        }
    }
    let start = args.offset.unwrap_or(0);
    let limit = args.max_files.unwrap_or(100).clamp(1, 1000);
    let mut indexed = 0;
    let mut unchanged = 0;
    let mut skipped = vec![];
    let mut processed = 0;
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    for path in files.iter().skip(start).take(limit) {
        if std::time::Instant::now() > deadline {
            break;
        }
        processed += 1;
        let outcome = (|| -> Result<(), String> {
            let bytes = fs::read(advanced::input(folders, &args.folder, path)?)
                .map_err(|e| e.to_string())?;
            let hash = writing::hash(&bytes);
            let old: Option<String> = db
                .query_row("SELECT sha FROM documents WHERE path=?", [path], |r| {
                    r.get(0)
                })
                .ok();
            if old.as_deref() == Some(&hash) {
                unchanged += 1;
                return Ok(());
            }
            let units = documents::units(folders, &args.folder, path)?;
            let tx = db.transaction().map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM search WHERE path=?", [path])
                .map_err(|e| e.to_string())?;
            for u in units.iter() {
                tx.execute(
                    "INSERT INTO search(path,page,sheet,sha,text) VALUES(?,?,?,?,?)",
                    params![path, u.page as i64, u.sheet, hash, u.text],
                )
                .map_err(|e| e.to_string())?;
            }
            tx.execute(
                "INSERT OR REPLACE INTO documents(path,sha,updated) VALUES(?,?,?)",
                params![path, hash, state::now() as i64],
            )
            .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            indexed += 1;
            Ok(())
        })();
        if let Err(error) = outcome {
            if skipped.len() < 20 {
                skipped.push(json!({"path":path,"reason":error}))
            }
        }
    }
    let next = start + processed;
    Ok(
        json!({"indexed":indexed,"unchanged":unchanged,"processed":processed,"files":files.len(),"next_offset":(next<files.len()).then_some(next),"skipped":skipped,"tree_truncated":truncated,"persistent":true,"engine":"SQLite FTS5, local, sans modèle ni API"}),
    )
}
pub fn query(folders: &[SharedFolder], args: QueryArgs) -> Result<Value, String> {
    if args.query.is_empty() || args.query.len() > 1000 {
        return Err("Recherche de 1 à 1000 octets".into());
    }
    let query = args
        .query
        .split_whitespace()
        .take(12)
        .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ");
    let db = database(folders, &args.folder)?;
    let mut statement=db.prepare("SELECT path,page,sheet,sha,snippet(search,4,'','',' … ',35) FROM search WHERE search MATCH ? ORDER BY rank LIMIT ?").map_err(|e|e.to_string())?;
    let rows = statement
        .query_map(
            params![query, args.limit.unwrap_or(20).clamp(1, 100) as i64],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;
    let mut results = vec![];
    let mut stale = vec![];
    let mut hashes = HashMap::new();
    for row in rows {
        let (path, page, sheet, sha, snippet) = row.map_err(|e| e.to_string())?;
        let hash = hashes.entry(path.clone()).or_insert_with(|| {
            advanced::input(folders, &args.folder, &path)
                .ok()
                .and_then(|p| fs::read(p).ok())
                .map(|b| writing::hash(&b))
        });
        if hash.as_deref() != Some(&sha) {
            stale.push(path);
            continue;
        }
        let citation_id = writing::hash(format!("{path}:{sha}:{page}").as_bytes());
        results.push(json!({"path":path,"page":page,"sheet":sheet,"sha256":sha,"snippet":snippet,"citation_id":citation_id,"source_verified":true}));
    }
    Ok(
        json!({"matches":results,"stale":stale,"refresh_required":!stale.is_empty(),"hint":"Utiliser les empreintes dans citations de create_document ; relancer index_folder après ajout ou modification de documents."}),
    )
}
pub fn status(folders: &[SharedFolder], folder: &str) -> Result<Value, String> {
    let db = database(folders, folder)?;
    let count: i64 = db
        .query_row("SELECT count(*) FROM documents", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    Ok(json!({"documents":count,"persistent":true}))
}
