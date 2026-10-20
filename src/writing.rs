use crate::library::{self, SharedFolder};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    sync::Mutex,
};

static WRITE_LOCK: Mutex<()> = Mutex::new(());
fn create_without_replace(source: &Path, target: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
        let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        fs::hard_link(source, target).map_err(|e| e.to_string())
    }
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn hash_file(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut state = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        state.update(&buffer[..n]);
    }
    Ok(format!("{:x}", state.finalize()))
}

pub fn destination(
    folders: &[SharedFolder],
    folder: &str,
    relative: &str,
) -> Result<PathBuf, String> {
    library::validate_relative(relative)?;
    let shared = library::select_folder(folders, folder)?;
    if !shared.writable {
        return Err("Écriture désactivée : activez-la pour ce dossier dans l'application, puis redémarrez le MCP".into());
    }
    let path = Path::new(relative);
    if crate::history::reserved(path) {
        return Err("Le dossier d'historique est réservé".into());
    }
    if relative.is_empty()
        || relative.contains('\0')
        || relative.contains(':')
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err("Chemin de sortie relatif invalide".into());
    }
    let name = path.file_name().ok_or("Nom de fichier requis")?;
    let parent = path.parent().unwrap_or(Path::new(""));
    let parent = library::resolve(folders, folder, &parent.to_string_lossy())?;
    if !parent.is_dir() {
        return Err("Le dossier de destination doit exister".into());
    }
    let target = parent.join(name);
    if target.exists() {
        return library::resolve(folders, folder, relative);
    }
    Ok(target)
}

pub fn commit(
    folders: &[SharedFolder],
    folder: &str,
    relative: &str,
    bytes: &[u8],
    expected_hash: Option<&str>,
) -> Result<Value, String> {
    crate::v1::jobs::check_write()?;
    if bytes.len() as u64 > crate::limits::DOCUMENT_BYTES {
        return Err("La sortie dépasse 100 Mio".into());
    }
    let _lock = WRITE_LOCK
        .lock()
        .map_err(|_| "Verrou d'écriture indisponible")?;
    crate::v1::jobs::check_write()?;
    let target = destination(folders, folder, relative)?;
    let existed = target.exists();
    if existed {
        if fs::metadata(&target).map_err(|e| e.to_string())?.len() > crate::limits::DOCUMENT_BYTES {
            return Err("Original trop volumineux pour une modification sauvegardée".into());
        }
        if !target.is_file() {
            return Err("La destination n'est pas un fichier".into());
        }
        let expected = expected_hash.ok_or(
            "Le fichier existe déjà : fournir son sha256 actuel pour autoriser la modification",
        )?;
        if hash(&fs::read(&target).map_err(|e| e.to_string())?) != expected {
            return Err("Conflit : le fichier a changé. Relisez-le avant de le modifier".into());
        }
    } else if expected_hash.is_some() {
        return Err("Le fichier à modifier n'existe plus".into());
    }
    let temporary = target
        .parent()
        .unwrap()
        .join(format!(".mcp-win-{}.tmp", uuid::Uuid::new_v4().simple()));
    let outcome = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        let mut backup = None;
        if existed {
            let previous = fs::read(&target).map_err(|e| e.to_string())?;
            if hash(&previous) != expected_hash.unwrap() {
                return Err("Conflit : le fichier a changé pendant la génération".into());
            }
            let root = library::select_folder(folders, folder)?
                .path
                .canonicalize()
                .map_err(|e| e.to_string())?;
            backup = Some(crate::history::snapshot(&root, relative, &previous)?);
            crate::v1::jobs::journal_intent(
                folders,
                folder,
                relative,
                &hash(bytes),
                expected_hash,
                backup.as_deref(),
            )?;
            fs::rename(&temporary, &target).map_err(|e| e.to_string())?;
        } else {
            crate::v1::jobs::journal_intent(folders, folder, relative, &hash(bytes), None, None)?;
            create_without_replace(&temporary, &target)
                .map_err(|e| format!("Création impossible sans écrasement : {e}"))?;
        }
        Ok(
            json!({"path":relative,"bytes":bytes.len(),"sha256":hash(bytes),"created":!existed,"previous_version":backup}),
        )
    })();
    let _ = fs::remove_file(temporary);
    outcome
}

pub fn remove_created(
    folders: &[SharedFolder],
    folder: &str,
    path: &str,
    expected: &str,
) -> Result<Value, String> {
    let _lock = WRITE_LOCK.lock().map_err(|_| "Écriture occupée")?;
    let target = destination(folders, folder, path)?;
    let metadata = fs::symlink_metadata(&target).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Annulation : fichier ordinaire requis".into());
    }
    let bytes = fs::read(&target).map_err(|e| e.to_string())?;
    if hash(&bytes) != expected {
        return Err("Le fichier a changé : annulation refusée".into());
    }
    let root = library::select_folder(folders, folder)?
        .path
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let backup = crate::history::snapshot(&root, path, &bytes)?;
    fs::remove_file(&target).map_err(|e| e.to_string())?;
    Ok(json!({"path":path,"removed":true,"previous_version":backup}))
}

pub fn text(
    folders: &[SharedFolder],
    folder: &str,
    path: &str,
    content: &str,
    expected_hash: Option<&str>,
) -> Result<Value, String> {
    destination(folders, folder, path)?;
    if content.len() > 2 * 1024 * 1024 {
        return Err("Texte limité à 2 Mo".into());
    }
    let ext = Path::new(path)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if ![
        "txt", "md", "csv", "tsv", "json", "xml", "svg", "html", "css", "js", "ts", "py", "rs",
        "yaml", "yml", "toml", "typ", "log",
    ]
    .contains(&ext.as_str())
    {
        return Err("Ce format nécessite un générateur dédié. Utilisez create_document ou write_spreadsheet".into());
    }
    if ext == "json" {
        serde_json::from_str::<Value>(content).map_err(|e| format!("JSON invalide : {e}"))?;
    }
    if ext == "xml" || ext == "svg" {
        let xml = roxmltree::Document::parse(content).map_err(|e| format!("XML invalide : {e}"))?;
        if ext == "svg" && xml.root_element().tag_name().name() != "svg" {
            return Err("Racine SVG requise".into());
        }
    }
    commit(folders, folder, path, content.as_bytes(), expected_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writing_requires_permission_and_correct_version() {
        let temp = tempfile::tempdir().unwrap();
        let mut folders = vec![SharedFolder {
            id: "test".into(),
            path: temp.path().into(),
            writable: false,
        }];
        assert!(text(&folders, "test", "note.txt", "a", None).is_err());
        folders[0].writable = true;
        let created = text(&folders, "test", "note.txt", "a", None).unwrap();
        assert!(text(&folders, "test", "note.txt", "b", None).is_err());
        assert!(text(&folders, "test", "note.txt", "b", Some("wrong")).is_err());
        text(
            &folders,
            "test",
            "note.txt",
            "b",
            created["sha256"].as_str(),
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(temp.path().join("note.txt")).unwrap(),
            "b"
        );
        assert!(text(&folders, "test", "../escape.txt", "x", None).is_err());
        assert!(text(&folders, "test", "note.txt:secret", "x", None).is_err());
        assert!(text(&folders, "test", "invalid.json", "{bad", None).is_err());
    }
}
