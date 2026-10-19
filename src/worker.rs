use crate::library::SharedFolder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
#[derive(Serialize, Deserialize)]
pub enum Operation {
    Archive(crate::archives::Command),
    InspectForm {
        folder: String,
        path: String,
        offset: usize,
        limit: usize,
    },
    FillForm(crate::pdf_forms::FillArgs),
    CombinePdf {
        folder: String,
        inputs: Vec<crate::pdf_ops::PdfInput>,
        output_path: String,
        flatten: bool,
    },
}
#[derive(Serialize, Deserialize)]
struct Request {
    folders: Vec<SharedFolder>,
    operation: Operation,
}
fn execute(folders: &[SharedFolder], operation: Operation) -> Result<Value, String> {
    match operation {
        Operation::Archive(command) => crate::archives::execute(folders, command),
        Operation::InspectForm {
            folder,
            path,
            offset,
            limit,
        } => crate::pdf_forms::inspect_window(folders, &folder, &path, offset, limit),
        Operation::FillForm(args) => crate::pdf_forms::fill(folders, args),
        Operation::CombinePdf {
            folder,
            inputs,
            output_path,
            flatten,
        } => crate::pdf_ops::combine(folders, &folder, &inputs, &output_path, flatten),
    }
}
// Parse untrusted archive/PDF headers and render forms outside the desktop process.
pub fn worker_main() -> bool {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_none_or(|a| a != "--mcp-worker") {
        return false;
    }
    if args.len() != 2 {
        std::process::exit(2);
    }
    let result = (|| {
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(2_000_001)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 2_000_000 {
            return Err("Requête worker trop volumineuse".into());
        }
        let req: Request = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        execute(&req.folders, req.operation)
    })();
    let result = serde_json::to_vec(&result).unwrap_or_default();
    if result.len() > 2_000_000 || std::io::stdout().write_all(&result).is_err() {
        std::process::exit(3);
    }
    true
}
pub async fn run(folders: Vec<SharedFolder>, operation: Operation) -> Result<Value, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let bytes = serde_json::to_vec(&Request { folders, operation }).map_err(|e| e.to_string())?;
    if bytes.len() > 2_000_000 {
        return Err("Requête worker trop volumineuse".into());
    }
    let mut command =
        tokio::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command
        .arg("--mcp-worker")
        .kill_on_drop(true)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let _job = crate::process_job::ProcessJob::attach(&child)?;
    let mut stdin = child.stdin.take().ok_or("Entrée worker indisponible")?;
    let stdout = child.stdout.take().ok_or("Sortie worker indisponible")?;
    // Pipe handles are private to these processes; sharing TEMP cannot alter folder permissions.
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        stdin.write_all(&bytes).await.map_err(|e| e.to_string())?;
        stdin.shutdown().await.map_err(|e| e.to_string())?;
        drop(stdin);
        let mut output = Vec::new();
        stdout
            .take(2_000_001)
            .read_to_end(&mut output)
            .await
            .map_err(|e| e.to_string())?;
        if output.len() > 2_000_000 {
            let _ = child.kill().await;
            return Err("Résultat worker trop volumineux".into());
        }
        let status = child.wait().await.map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(
                "Fichier invalide ou traitement interrompu par les limites de sécurité".into(),
            );
        }
        serde_json::from_slice::<Result<Value, String>>(&output).map_err(|e| e.to_string())?
    })
    .await;
    match outcome {
        Ok(result) => result,
        Err(_) => {
            let _ = child.kill().await;
            Err(
                "Traitement interrompu après 30 secondes ; vérifier les sorties avant de relancer"
                    .into(),
            )
        }
    }
}
