//! Local file ownership for the embedded office editors. Documents never become WebView code.
use base64::Engine;
use serde_json::{Value, json};
use std::io::Read;
use std::{collections::HashMap, path::PathBuf, sync::Mutex};
use tauri::State;

#[derive(Clone)]
struct FileTicket {
    path: Option<PathBuf>,
    sha256: Option<String>,
    kind: String,
    shared: Option<(String, String)>,
}
#[derive(Default)]
pub struct OfficeSuite(Mutex<HashMap<String, FileTicket>>, Mutex<Lifecycle>);
#[derive(Default)]
struct Lifecycle {
    dirty: Vec<String>,
    close_requested: bool,
    allow_close: bool,
}
impl OfficeSuite {
    pub fn has_unsaved(&self) -> bool {
        self.1.lock().map(|s| !s.dirty.is_empty()).unwrap_or(true)
    }
    pub fn prevent_close(&self) -> bool {
        if let Ok(mut status) = self.1.lock() {
            if !status.allow_close && !status.dirty.is_empty() {
                status.close_requested = true;
                return true;
            }
        }
        false
    }
}
#[tauri::command]
pub fn office_lifecycle(dirty: Vec<String>, state: State<'_, OfficeSuite>) -> Result<bool, String> {
    let files = state.0.lock().map_err(|_| "Editeurs indisponibles")?;
    let mut status = state.1.lock().map_err(|_| "Editeurs indisponibles")?;
    status.dirty = dirty
        .into_iter()
        .filter(|id| files.contains_key(id))
        .take(12)
        .collect();
    let close = status.close_requested;
    status.close_requested = false;
    Ok(close)
}
#[tauri::command]
pub fn office_close_app(
    window: tauri::WebviewWindow,
    state: State<'_, OfficeSuite>,
) -> Result<(), String> {
    state
        .1
        .lock()
        .map_err(|_| "Editeurs indisponibles")?
        .allow_close = true;
    window.close().map_err(|e| e.to_string())
}
fn kind(path: &str) -> Result<&'static str, String> {
    match path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "docx" | "doc" | "docm" | "dotx" | "rtf" | "odt" | "txt" | "md" | "html" => Ok("word"),
        "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" | "csv" | "tsv" => Ok("grid"),
        "pptx" | "pptm" | "potx" | "odp" => Ok("deck"),
        "pdf" => Ok("pdf"),
        "designcraft" | "idml" | "indd" | "indt" => Ok("design"),
        _ => Err("Format bureautique non pris en charge".into()),
    }
}
fn output_allowed(name: &str) -> bool {
    matches!(
        name.rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "docx"
            | "docm"
            | "dotx"
            | "pdf"
            | "rtf"
            | "odt"
            | "txt"
            | "md"
            | "html"
            | "xlsx"
            | "xlsm"
            | "csv"
            | "tsv"
            | "pptx"
            | "odp"
            | "png"
            | "jpg"
            | "svg"
            | "designcraft"
            | "idml"
            | "epub"
    )
}
fn save_name(editor: &str, name: &str) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let replacement = match (editor, ext.as_str()) {
        ("word", "doc") => Some("docx"),
        ("grid", "xls" | "xlsb" | "ods") => Some("xlsx"),
        ("deck", "pptm" | "potx" | "odp") => Some("pptx"),
        ("design", "idml" | "indd" | "indt") => Some("designcraft"),
        _ => None,
    };
    replacement
        .map(|ext| {
            PathBuf::from(name)
                .with_extension(ext)
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| name.into())
}
impl OfficeSuite {
    fn insert(&self, file: FileTicket) -> Result<String, String> {
        let mut files = self.0.lock().map_err(|_| "Editeurs indisponibles")?;
        if files.len() >= 12 {
            return Err("Fermez un document avant d’en ouvrir un autre".into());
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        files.insert(id.clone(), file);
        Ok(id)
    }
    fn open(&self, path: PathBuf, shared: Option<(String, String)>) -> Result<Value, String> {
        let name = path
            .file_name()
            .ok_or("Nom de fichier invalide")?
            .to_string_lossy()
            .to_string();
        let editor = kind(&name)?.to_string();
        let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.len() > crate::limits::DOCUMENT_BYTES {
            return Err("Fichier invalide ou superieur a 100 Mio".into());
        }
        let bytes = read_bounded(&path, crate::limits::DOCUMENT_BYTES)?;
        let sha = crate::writing::hash(&bytes);
        let id = self.insert(FileTicket {
            path: Some(path),
            sha256: Some(sha),
            kind: editor.clone(),
            shared,
        })?;
        Ok(
            json!({"ticket":id,"name":name,"save_name":save_name(&editor,&name),"kind":editor,"data":base64::engine::general_purpose::STANDARD.encode(bytes)}),
        )
    }
}
fn read_bounded(path: &std::path::Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Fichier trop volumineux".into());
    }
    Ok(bytes)
}
#[tauri::command]
pub fn office_new(kind: String, state: State<'_, OfficeSuite>) -> Result<Value, String> {
    let name = match kind.as_str() {
        "word" => "Document.docx",
        "grid" => "Classeur.xlsx",
        "deck" => "Presentation.pptx",
        "pdf" => "Document.pdf",
        "design" => "Composition.designcraft",
        _ => return Err("Editeur inconnu".into()),
    };
    let id = state.insert(FileTicket {
        path: None,
        sha256: None,
        kind: kind.clone(),
        shared: None,
    })?;
    Ok(json!({"ticket":id,"name":name,"kind":kind}))
}
#[tauri::command]
pub async fn office_pick(state: State<'_, OfficeSuite>) -> Result<Value, String> {
    let Some(path) = rfd::AsyncFileDialog::new()
        .add_filter(
            "Documents",
            &[
                "docx",
                "doc",
                "rtf",
                "odt",
                "xlsx",
                "xls",
                "ods",
                "csv",
                "pptx",
                "pdf",
                "designcraft",
                "idml",
            ],
        )
        .pick_file()
        .await
    else {
        return Ok(Value::Null);
    };
    state.open(path.path().to_path_buf(), None)
}
#[tauri::command]
pub async fn office_asset() -> Result<Value, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .add_filter(
            "Images",
            &["png", "jpg", "jpeg", "svg", "webp", "bmp", "tif", "tiff"],
        )
        .pick_file()
        .await
    else {
        return Ok(Value::Null);
    };
    let path = file.path();
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 20 * 1024 * 1024 {
        return Err("Image superieure a 20 Mio".into());
    }
    Ok(
        json!({"name":file.file_name(),"data":base64::engine::general_purpose::STANDARD.encode(read_bounded(path,20*1024*1024)?)}),
    )
}
#[tauri::command]
pub async fn office_ocr(
    ticket: String,
    page: usize,
    state: State<'_, OfficeSuite>,
) -> Result<Value, String> {
    let file = state
        .0
        .lock()
        .map_err(|_| "Editeurs indisponibles")?
        .get(&ticket)
        .cloned()
        .ok_or("Document ferme")?;
    let path = file
        .path
        .ok_or("Enregistrez le PDF avant la reconnaissance de texte")?;
    if file.kind != "pdf" {
        return Err("La reconnaissance de texte concerne les PDF numerises".into());
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Nom invalide")?
        .to_string();
    let folders = vec![crate::library::SharedFolder {
        id: "office-ocr".into(),
        path: path.parent().ok_or("Dossier absent")?.into(),
        writable: false,
    }];
    crate::ocr::recognize(
        folders,
        crate::ocr::OcrArgs {
            folder: "office-ocr".into(),
            path: name,
            page: Some(page),
            limit: Some(50000),
            ..Default::default()
        },
    )
    .await
}
#[tauri::command]
pub fn office_open_shared(
    folder: String,
    path: String,
    state: State<'_, OfficeSuite>,
    desktop: State<'_, crate::desktop::Desktop>,
) -> Result<Value, String> {
    let folders = desktop.shared_folders();
    let id = crate::library::select_folder(&folders, &folder)?.id.clone();
    let resolved = crate::library::resolve(&folders, &id, &path)?;
    state.open(resolved, Some((id, path)))
}
#[tauri::command]
pub async fn office_save(
    ticket: String,
    name: String,
    data: String,
    save_as: bool,
    state: State<'_, OfficeSuite>,
    desktop: State<'_, crate::desktop::Desktop>,
) -> Result<Value, String> {
    if data.len() > 140_000_000 {
        return Err("Fichier superieur a 100 Mio".into());
    }
    let file = state
        .0
        .lock()
        .map_err(|_| "Editeurs indisponibles")?
        .get(&ticket)
        .cloned()
        .ok_or("Document ferme")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| "Donnees invalides")?;
    if bytes.len() as u64 > crate::limits::DOCUMENT_BYTES || !output_allowed(&name) {
        return Err("Format ou taille non autorises".into());
    }
    let same_ext = file
        .path
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|x| x.to_str())
        .map(|x| {
            name.to_ascii_lowercase()
                .ends_with(&format!(".{}", x.to_ascii_lowercase()))
        })
        .unwrap_or(false);
    let (path, expected) = if !save_as && same_ext && file.path.is_some() {
        (
            file.path.clone().ok_or("Chemin absent")?,
            file.sha256.clone(),
        )
    } else {
        let suggested = std::path::Path::new(&name)
            .file_name()
            .and_then(|x| x.to_str())
            .ok_or("Nom invalide")?;
        let Some(selected) = rfd::AsyncFileDialog::new()
            .set_file_name(suggested)
            .save_file()
            .await
        else {
            return Ok(json!({"cancelled":true}));
        };
        let path = selected.path().to_path_buf();
        if !output_allowed(&path.to_string_lossy()) {
            return Err("Extension non autorisee".into());
        }
        if path.extension() != std::path::Path::new(suggested).extension() {
            return Err("Conservez l’extension du format exporte".into());
        }
        let hash = if path.exists() {
            Some(crate::writing::hash_file(&path)?)
        } else {
            None
        };
        (path, hash)
    };
    if file.path.as_ref() == Some(&path) {
        if let Some((folder, relative)) = &file.shared {
            let scopes = desktop.shared_folders();
            let source = crate::library::resolve(&scopes, folder, relative)?;
            if source != path || !scopes.iter().any(|f| f.id == *folder && f.writable) {
                return Err("Ce dossier partage est en lecture seule. Utilisez Enregistrer sous dans un autre dossier.".into());
            }
        }
    }
    let parent = path.parent().ok_or("Dossier absent")?;
    let base = path
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or("Nom invalide")?;
    let folders = vec![crate::library::SharedFolder {
        id: "office-local".into(),
        path: parent.into(),
        writable: true,
    }];
    let result = if file.path.as_ref() == Some(&path) {
        if let Some((folder, relative)) = &file.shared {
            crate::writing::commit(
                &desktop.shared_folders(),
                folder,
                relative,
                &bytes,
                expected.as_deref(),
            )?
        } else {
            crate::writing::commit(&folders, "office-local", base, &bytes, expected.as_deref())?
        }
    } else {
        crate::writing::commit(&folders, "office-local", base, &bytes, expected.as_deref())?
    };
    let sha = crate::writing::hash(&bytes);
    let saved_name = base.to_string();
    // Imported, read-only formats become a new editable file; PDF/image exports stay separate.
    let converted = file
        .path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .is_some_and(|original| {
            let working = save_name(&file.kind, original);
            working != original && std::path::Path::new(&working).extension() == path.extension()
        });
    let bound = same_ext
        || converted
        || (file.path.is_none() && kind(&name).ok() == Some(file.kind.as_str()));
    if bound {
        state
            .0
            .lock()
            .map_err(|_| "Editeurs indisponibles")?
            .insert(
                ticket,
                FileTicket {
                    path: Some(path.clone()),
                    sha256: Some(sha),
                    kind: file.kind,
                    shared: if file.path.as_ref() == Some(&path) {
                        file.shared
                    } else {
                        None
                    },
                },
            );
    }
    Ok(json!({"saved":true,"bound":bound,"name":saved_name,"result":result}))
}
#[tauri::command]
pub fn office_close(ticket: String, state: State<'_, OfficeSuite>) -> Result<(), String> {
    state
        .0
        .lock()
        .map_err(|_| "Editeurs indisponibles")?
        .remove(&ticket);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_or_non_file_inputs_never_create_tickets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.pdf");
        std::fs::File::create(&path)
            .unwrap()
            .set_len(crate::limits::DOCUMENT_BYTES + 1)
            .unwrap();
        let suite = OfficeSuite::default();
        assert!(suite.open(path, None).is_err());
        assert!(suite.open(dir.path().into(), None).is_err());
        assert!(suite.0.lock().unwrap().is_empty());
    }

    #[test]
    fn ticket_capacity_and_open_snapshot_are_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.txt");
        std::fs::write(&path, b"initial").unwrap();
        let suite = OfficeSuite::default();
        let opened = suite.open(path.clone(), None).unwrap();
        std::fs::write(&path, b"external change").unwrap();
        let files = suite.0.lock().unwrap();
        let file = files
            .get(opened["ticket"].as_str().unwrap())
            .unwrap()
            .clone();
        assert_ne!(file.sha256, Some(crate::writing::hash_file(&path).unwrap()));
        drop(files);
        for _ in 1..12 {
            suite.insert(file.clone()).unwrap();
        }
        assert!(suite.insert(file).is_err());
        assert_eq!(suite.0.lock().unwrap().len(), 12);
    }

    #[test]
    fn unsaved_documents_block_window_close_and_updates() {
        let suite = OfficeSuite::default();
        assert!(!suite.prevent_close());
        suite.1.lock().unwrap().dirty.push("document".into());
        assert!(suite.has_unsaved());
        assert!(suite.prevent_close());
        assert!(suite.1.lock().unwrap().close_requested);
        suite.1.lock().unwrap().allow_close = true;
        assert!(!suite.prevent_close());
    }
}
