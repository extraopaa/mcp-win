use crate::library::{self, SharedFolder};
use rmcp::schemars;
use rusqlite::{
    Connection, OpenFlags,
    hooks::{AuthAction, AuthContext, Authorization},
    limits::Limit,
    types::ValueRef,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    time::{Duration, Instant},
};

#[derive(Deserialize, schemars::JsonSchema)]
pub struct QueryArgs {
    #[serde(default)]
    pub folder: String,
    pub path: String,
    /// Omettre pour inspecter les tables et leurs définitions. SELECT/WITH seulement.
    pub query: Option<String>,
    /// Paramètres positionnels ? : chaînes, nombres, booléens ou null.
    #[serde(default)]
    pub parameters: Vec<Value>,
    pub max_rows: Option<usize>,
}

pub fn query(folders: &[SharedFolder], args: QueryArgs) -> Result<Value, String> {
    let path = library::resolve(folders, &args.folder, &args.path)?;
    let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > 4 * 1024 * 1024 * 1024 {
        return Err("Base SQLite requise, maximum 4 Gio".into());
    }
    let mut magic = [0; 16];
    fs::File::open(&path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .map_err(|e| e.to_string())?;
    if &magic != b"SQLite format 3\0" {
        return Err("Ce fichier n’est pas une base SQLite".into());
    }
    for suffix in ["-wal", "-journal"] {
        let mut side = path.as_os_str().to_os_string();
        side.push(suffix);
        if fs::metadata(side).is_ok_and(|m| m.len() > 0) {
            return Err("Base active avec journal : fermer son application ou partager un export SQLite cohérent avant la lecture".into());
        }
    }
    let raw = path.to_string_lossy();
    let raw = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    let mut uri = reqwest::Url::from_file_path(raw).map_err(|_| "Chemin de base invalide")?;
    uri.set_query(Some("mode=ro&immutable=1"));
    let conn = Connection::open_with_flags(
        uri.as_str(),
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    conn.busy_timeout(Duration::from_millis(100))
        .map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA query_only=ON; PRAGMA trusted_schema=OFF; PRAGMA temp_store=MEMORY; PRAGMA cache_size=-8192;").map_err(|e|e.to_string())?;
    for (key, value) in [
        (Limit::SQLITE_LIMIT_LENGTH, 2_000_000),
        (Limit::SQLITE_LIMIT_SQL_LENGTH, 64000),
        (Limit::SQLITE_LIMIT_COLUMN, 200),
        (Limit::SQLITE_LIMIT_EXPR_DEPTH, 100),
        (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 50),
        (Limit::SQLITE_LIMIT_ATTACHED, 0),
        (Limit::SQLITE_LIMIT_VDBE_OP, 200_000),
    ] {
        conn.set_limit(key, value).map_err(|e| e.to_string())?;
    }
    conn.authorizer(Some(|ctx: AuthContext<'_>| match ctx.action {
        AuthAction::Select | AuthAction::Read { .. } | AuthAction::Recursive => {
            Authorization::Allow
        }
        AuthAction::Function { function_name }
            if !["load_extension", "readfile", "writefile", "fts3_tokenizer"]
                .contains(&function_name.to_ascii_lowercase().as_str()) =>
        {
            Authorization::Allow
        }
        AuthAction::Pragma { pragma_name, .. }
            if [
                "table_info",
                "table_xinfo",
                "index_list",
                "index_info",
                "foreign_key_list",
                "table_list",
            ]
            .contains(&pragma_name) =>
        {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }))
    .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut ticks = 0;
    conn.progress_handler(
        1000,
        Some(move || {
            ticks += 1;
            ticks > 2000 || Instant::now() > deadline
        }),
    )
    .map_err(|e| e.to_string())?;
    let sql=args.query.unwrap_or_else(||"SELECT name,type,sql FROM sqlite_schema WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY name".into());
    if sql.len() > 60000 || args.parameters.len() > 100 {
        return Err("Requête trop volumineuse".into());
    }
    let sql = sql.trim().trim_end_matches(';').trim();
    let wrapped = format!(
        "SELECT * FROM ({sql}) LIMIT {}",
        args.max_rows.unwrap_or(50).clamp(1, 500) + 1
    );
    let mut stmt = conn
        .prepare(&wrapped)
        .map_err(|e| format!("SELECT en lecture seule requis : {e}"))?;
    if !stmt.readonly() {
        return Err("Requête modifiante interdite".into());
    }
    let columns = stmt
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let parameters = args
        .parameters
        .into_iter()
        .map(|v| match v {
            Value::Null => Ok(rusqlite::types::Value::Null),
            Value::Bool(b) => Ok(rusqlite::types::Value::Integer(i64::from(b))),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(rusqlite::types::Value::Integer(i))
                } else {
                    n.as_f64()
                        .map(rusqlite::types::Value::Real)
                        .ok_or("Nombre invalide")
                }
            }
            Value::String(s) => Ok(rusqlite::types::Value::Text(s)),
            _ => Err("Paramètre SQL scalaire requis"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut rows = stmt
        .query(rusqlite::params_from_iter(parameters))
        .map_err(|e| e.to_string())?;
    let max = args.max_rows.unwrap_or(50).clamp(1, 500);
    let mut result = Vec::new();
    let mut bytes = 0usize;
    let mut truncated = false;
    while let Some(row) = rows
        .next()
        .map_err(|e| format!("Requête interrompue ou invalide : {e}"))?
    {
        if result.len() >= max {
            truncated = true;
            break;
        }
        let mut cells = Vec::new();
        for i in 0..columns.len() {
            cells.push(match row.get_ref(i).map_err(|e| e.to_string())? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(n) => json!(n),
                ValueRef::Real(n) => json!(n),
                ValueRef::Text(t) => {
                    let s = String::from_utf8_lossy(t);
                    json!(s.chars().take(2000).collect::<String>())
                }
                ValueRef::Blob(b) => json!({"blob_bytes":b.len()}),
            });
        }
        let row = json!(cells);
        let size = row.to_string().len();
        if bytes + size > 128000 {
            truncated = true;
            break;
        }
        bytes += size;
        result.push(row);
    }
    Ok(
        json!({"columns":columns,"rows":result,"truncated":truncated,"read_only":true,"cell_text_limit":2000}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queries_schema_parameters_and_denies_mutations_or_external_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("data.db");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE invoices(id INTEGER,total REAL); INSERT INTO invoices VALUES(1,42.5),(2,99);").unwrap();
        drop(db);
        let folders = [SharedFolder {
            id: "test".into(),
            path: temp.path().into(),
            writable: true,
        }];
        let run = |sql: Option<&str>, parameters: Vec<Value>| {
            query(
                &folders,
                QueryArgs {
                    folder: "".into(),
                    path: "data.db".into(),
                    query: sql.map(str::to_owned),
                    parameters,
                    max_rows: Some(1),
                },
            )
        };
        assert_eq!(
            run(
                Some("SELECT total FROM invoices WHERE id=?"),
                vec![json!(1)]
            )
            .unwrap()["rows"][0][0],
            42.5
        );
        assert_eq!(run(None, vec![]).unwrap()["rows"][0][0], "invoices");
        let original = fs::read(&path).unwrap();
        for sql in [
            "DELETE FROM invoices",
            "SELECT 1; DELETE FROM invoices",
            "ATTACH 'other.db' AS x",
            "PRAGMA journal_mode=WAL",
            "SELECT load_extension('x')",
            "SELECT writefile('x','bad')",
            "SELECT * FROM readfile('x')",
        ] {
            assert!(run(Some(sql), vec![]).is_err(), "{sql}");
        }
        assert_eq!(fs::read(path).unwrap(), original);
        assert!(!temp.path().join("other.db").exists());
    }
}
