use crate::{
    advanced,
    library::{self, SharedFolder},
    writing,
};
use rmcp::schemars;
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs;
#[derive(Deserialize, schemars::JsonSchema)]
pub struct QueryArgs {
    #[serde(default)]
    pub folder: String,
    pub path: String,
    pub query: Option<String>,
    pub max_rows: Option<usize>,
}
fn select(sql: &str) -> Result<&str, String> {
    if sql.len() > 60000 || sql.contains('\0') {
        return Err("Requête trop longue ou invalide".into());
    }
    let sql = sql.trim().trim_end_matches(';').trim();
    let first = sql
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["select", "with"].contains(&first.as_str()) {
        return Err("SELECT/WITH unique requis".into());
    }
    let b = sql.as_bytes();
    let mut i = 0;
    let mut quote = 0u8;
    while i < b.len() {
        let c = b[i];
        if quote != 0 {
            if c == quote {
                if b.get(i + 1) == Some(&quote) {
                    i += 2;
                    continue;
                }
                quote = 0;
            }
            i += 1;
            continue;
        }
        if c == b'\'' || c == b'"' {
            quote = c;
        } else if c == b';' {
            return Err("Plusieurs instructions SQL interdites".into());
        } else if c == b'-' && b.get(i + 1) == Some(&b'-') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            return Err("Commentaires de bloc SQL non acceptés ; retirer les commentaires".into());
        }
        i += 1;
    }
    if quote != 0 {
        return Err("Chaîne SQL non terminée".into());
    }
    Ok(sql)
}
pub async fn query(folders: &[SharedFolder], args: QueryArgs) -> Result<Value, String> {
    use tokio::io::AsyncReadExt;
    let source = library::resolve(folders, &args.folder, &args.path)?;
    let meta = fs::metadata(&source).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > 4 * 1024 * 1024 * 1024 {
        return Err("Base DuckDB requise (4 Gio max)".into());
    }
    let mut wal = source.as_os_str().to_os_string();
    wal.push(".wal");
    if fs::metadata(wal).is_ok_and(|m| m.len() > 0) {
        return Err("Base DuckDB active : fermer l’application ou effectuer un CHECKPOINT/export avant partage".into());
    }
    let requested=args.query.unwrap_or_else(||"SELECT table_schema,table_name,column_name,data_type FROM information_schema.columns WHERE table_catalog=current_database() ORDER BY table_schema,table_name,ordinal_position".into());
    let sql = select(&requested)?;
    let max = args.max_rows.unwrap_or(50).clamp(1, 500);
    let sql = format!("SELECT * FROM ({sql}) LIMIT {};", max + 1);
    let engine = advanced::engine_path("duckdb/duckdb.exe")?;
    if writing::hash(&fs::read(&engine).map_err(|e| e.to_string())?)
        != "2c6a856516a9efb863482a9146242eba5ad919029a082ff773f4770ae0a7816b"
    {
        return Err("Empreinte du moteur DuckDB incorrecte".into());
    }
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let error_path = temp.path().join("error.txt");
    let mut command = tokio::process::Command::new(engine);
    command
        .args(["-no-init", "-readonly", "-safe", "-batch", "-bail", "-json"])
        .arg(&source)
        .arg(sql)
        .current_dir(temp.path())
        .env_clear()
        .env("HOME", temp.path())
        .env("USERPROFILE", temp.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::from(
            fs::File::create(&error_path).map_err(|e| e.to_string())?,
        ))
        .kill_on_drop(true);
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let _job = crate::process_job::ProcessJob::attach(&child)?;
    let stdout = child.stdout.take().ok_or("Sortie DuckDB manquante")?;
    let output = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let mut out = Vec::new();
        stdout
            .take(256001)
            .read_to_end(&mut out)
            .await
            .map_err(|e| e.to_string())?;
        if out.len() > 256000 {
            let _ = child.kill().await;
            return Err(
                "Résultat DuckDB trop volumineux : limiter les colonnes/lignes ou utiliser substr"
                    .into(),
            );
        }
        let status = child.wait().await.map_err(|e| e.to_string())?;
        if !status.success() {
            let error = fs::read_to_string(&error_path).unwrap_or_default();
            return Err(format!(
                "DuckDB lecture seule : {}",
                error.chars().take(3000).collect::<String>()
            ));
        }
        Ok(out)
    })
    .await;
    let out = match output {
        Ok(out) => out?,
        Err(_) => {
            let _ = child.kill().await;
            return Err("Requête DuckDB interrompue après 10 secondes".into());
        }
    };
    let mut rows = if out.iter().all(u8::is_ascii_whitespace) {
        Vec::new()
    } else {
        serde_json::from_slice::<Vec<Value>>(&out).map_err(|e| e.to_string())?
    };
    let truncated = rows.len() > max;
    rows.truncate(max);
    fn compact(v: &mut Value) {
        match v {
            Value::String(s) => {
                if s.chars().count() > 2000 {
                    *s = s.chars().take(2000).collect();
                }
            }
            Value::Array(a) => {
                for v in a {
                    compact(v)
                }
            }
            Value::Object(o) => {
                for v in o.values_mut() {
                    compact(v)
                }
            }
            _ => {}
        }
    }
    for row in &mut rows {
        compact(row);
    }
    Ok(
        json!({"rows":rows,"truncated":truncated,"read_only":true,"cell_text_limit":2000,"engine":"duckdb-1.5.6"}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn reads_database_and_blocks_external_access() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("data.duckdb");
        let exe = advanced::engine_path("duckdb/duckdb.exe").unwrap();
        let mut cmd = std::process::Command::new(exe);
        cmd.arg(&path).arg("CREATE TABLE sales(id INTEGER, amount DOUBLE); INSERT INTO sales VALUES(1,12.5),(2,45.0)").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        assert!(cmd.status().unwrap().success());
        let folders = [SharedFolder {
            id: "test".into(),
            path: temp.path().into(),
            writable: false,
        }];
        let original = fs::read(&path).unwrap();
        let run = |q: Option<&str>| QueryArgs {
            folder: "".into(),
            path: "data.duckdb".into(),
            query: q.map(str::to_owned),
            max_rows: Some(1),
        };
        let result = query(&folders, run(Some("SELECT amount FROM sales ORDER BY id")))
            .await
            .unwrap();
        assert_eq!(result["rows"][0]["amount"], 12.5);
        assert_eq!(result["truncated"], true);
        assert!(
            query(&folders, run(None)).await.unwrap()["rows"]
                .as_array()
                .is_some()
        );
        for sql in [
            "DELETE FROM sales",
            "SELECT 1; DELETE FROM sales",
            "SELECT * FROM read_csv('C:/Windows/win.ini')",
            "SELECT * FROM read_csv('https://example.com/x.csv')",
            "SELECT getenv('USERPROFILE')",
        ] {
            assert!(query(&folders, run(Some(sql))).await.is_err(), "{sql}");
        }
        assert_eq!(fs::read(path).unwrap(), original);
    }
}
