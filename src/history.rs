use crate::{
    library::{self, SharedFolder},
    writing,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
pub const DIRECTORY: &str = ".mcp-history";
pub fn reserved(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(DIRECTORY)
            || c.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(".mcp-private")
    })
}
fn directory(root: &Path, create: bool) -> Result<PathBuf, String> {
    let dir = root.join(DIRECTORY);
    if dir.exists() {
        let m = fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return Err("Le dossier d'historique ne doit pas être un lien/jonction".into());
            }
        }
        if m.file_type().is_symlink() || !m.is_dir() {
            return Err("Historique local invalide".into());
        }
    } else if create {
        fs::create_dir(&dir).map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_ATTRIBUTE_HIDDEN, SetFileAttributesW,
            };
            let path: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
            if unsafe { SetFileAttributesW(path.as_ptr(), FILE_ATTRIBUTE_HIDDEN) } == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
    }
    Ok(dir)
}
#[derive(Serialize, Deserialize)]
struct Version {
    id: String,
    path: String,
    sha256: String,
    bytes: usize,
    timestamp: u64,
    #[serde(default)]
    created_at_ms: u64,
}
pub fn snapshot(root: &Path, path: &str, bytes: &[u8]) -> Result<String, String> {
    let dir = directory(root, true)?;
    let size = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum::<u64>();
    if size + bytes.len() as u64 > 512 * 1024 * 1024 {
        return Err(
            "Historique plein (512 Mo) : libérer de l'espace avant de modifier ; original conservé"
                .into(),
        );
    }
    let id = uuid::Uuid::new_v4().simple().to_string();
    let version = Version {
        id: id.clone(),
        path: path.replace('\\', "/"),
        sha256: writing::hash(bytes),
        bytes: bytes.len(),
        created_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    let bin = dir.join(format!("{id}.bin"));
    let meta = dir.join(format!("{id}.json"));
    let outcome = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&bin)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&meta)
            .map_err(|e| e.to_string())?;
        f.write_all(&serde_json::to_vec(&version).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        Ok(id.clone())
    })();
    if outcome.is_err() {
        let _ = fs::remove_file(bin);
        let _ = fs::remove_file(meta);
    }
    outcome
}
pub fn list(folders: &[SharedFolder], folder: &str, path: Option<&str>) -> Result<Value, String> {
    let shared = library::select_folder(folders, folder)?;
    let root = shared.path.canonicalize().map_err(|e| e.to_string())?;
    let dir = directory(&root, false)?;
    let mut versions = Vec::new();
    if dir.is_dir() {
        for e in fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
        {
            if e.path().extension().is_some_and(|x| x == "json")
                && e.file_type().is_ok_and(|t| t.is_file())
                && e.metadata().is_ok_and(|m| m.len() < 8192)
            {
                if let Ok(v) = serde_json::from_slice::<Version>(
                    &fs::read(e.path()).map_err(|e| e.to_string())?,
                ) {
                    if path.is_none_or(|p| p.replace('\\', "/") == v.path) {
                        versions.push(v);
                    }
                }
            }
        }
    }
    versions.sort_by(|a, b| {
        b.created_at_ms
            .max(b.timestamp.saturating_mul(1000))
            .cmp(&a.created_at_ms.max(a.timestamp.saturating_mul(1000)))
            .then(b.id.cmp(&a.id))
    });
    let total = versions.len();
    versions.truncate(200);
    Ok(json!({"versions":versions,"total":total,"truncated":total>200}))
}
pub fn restore(
    folders: &[SharedFolder],
    folder: &str,
    id: &str,
    expected: &str,
) -> Result<Value, String> {
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Identifiant de version invalide".into());
    }
    let shared = library::select_folder(folders, folder)?;
    let root = shared.path.canonicalize().map_err(|e| e.to_string())?;
    let dir = directory(&root, false)?;
    let meta = dir.join(format!("{id}.json"));
    let bin = dir.join(format!("{id}.bin"));
    for file in [&meta, &bin] {
        let m = fs::symlink_metadata(file).map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return Err("Version liée par jonction interdite".into());
            }
        }
        if m.file_type().is_symlink() || !m.is_file() || m.len() > 25 * 1024 * 1024 {
            return Err("Version invalide".into());
        }
    }
    if fs::metadata(&meta).map_err(|e| e.to_string())?.len() > 8192 {
        return Err("Métadonnées invalides".into());
    }
    let v: Version = serde_json::from_slice(&fs::read(meta).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let bytes = fs::read(bin).map_err(|e| e.to_string())?;
    if v.id != id || writing::hash(&bytes) != v.sha256 {
        return Err("Version altérée : restauration refusée".into());
    }
    writing::commit(folders, folder, &v.path, &bytes, Some(expected))
}

pub fn restore_local(folders: &[SharedFolder], folder: &str, id: &str) -> Result<Value, String> {
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Version invalide".into());
    }
    let root = library::resolve(folders, folder, "")?;
    let dir = directory(&root, false)?;
    let meta = dir.join(format!("{id}.json"));
    if fs::symlink_metadata(&meta)
        .map_err(|e| e.to_string())?
        .len()
        > 8192
    {
        return Err("Métadonnées trop volumineuses".into());
    }
    let metadata = fs::symlink_metadata(&meta).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 8192 {
        return Err("Métadonnées de version invalides".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Métadonnées liées par jonction interdites".into());
        }
    }
    let v: Version = serde_json::from_slice(&fs::read(meta).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let current = crate::advanced::input(folders, folder, &v.path)?;
    let expected = writing::hash(&fs::read(current).map_err(|e| e.to_string())?);
    restore(folders, folder, id, &expected)
}
