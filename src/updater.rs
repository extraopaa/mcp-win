//! Signed full-package updates for the portable Windows distribution.
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::Mutex,
};
use tauri::State;

const ENDPOINT: &str = "https://github.com/extraopaa/mcp-win/releases/latest/download/update.json";
const PUBLIC_KEY: &str = include_str!("../assets/updater-public-key.txt");
const MAX_ARCHIVE: u64 = 768 * 1024 * 1024;
const MAX_EXPANDED: u64 = 2 * 1024 * 1024 * 1024;
pub fn cleanup() {
    let Ok(root) = crate::v1::state::root().map(|p| p.join("updates")) else {
        return;
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        if !entry.file_name().to_string_lossy().starts_with("update-") {
            continue;
        }
        let Ok(m) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if !m.is_dir() || m.file_type().is_symlink() {
            continue;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                continue;
            }
        }
        let success = fs::read(entry.path().join("result.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Result<(), String>>(&b).ok())
            .is_some_and(|r| r.is_ok());
        let expired = m
            .modified()
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|t| t.as_secs() > 7 * 86400);
        if success || expired {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: u32,
    pub version: String,
    pub target: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}
struct Pending {
    directory: PathBuf,
    manifest: Manifest,
}
struct Inner {
    status: String,
    version: Option<String>,
    error: Option<String>,
    progress: u64,
    last_check: u64,
    busy: bool,
    pending: Option<Pending>,
}
pub struct Updater {
    inner: Mutex<Inner>,
}
impl Default for Updater {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner {
                status: "idle".into(),
                version: None,
                error: None,
                progress: 0,
                last_check: 0,
                busy: false,
                pending: None,
            }),
        }
    }
}
fn snapshot(state: &Updater) -> Value {
    let s = state.inner.lock().unwrap();
    json!({"status":s.status,"version":s.version,"error":s.error,"progress":s.progress,"current_version":env!("CARGO_PKG_VERSION")})
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .https_only(true)
        .user_agent(concat!("MCP-Win/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::custom(|a| {
            let url = a.url();
            if a.previous().len() > 5
                || url.scheme() != "https"
                || !matches!(
                    url.host_str(),
                    Some(
                        "github.com"
                            | "release-assets.githubusercontent.com"
                            | "objects.githubusercontent.com"
                    )
                )
            {
                a.error("Redirection de mise à jour refusée")
            } else {
                a.follow()
            }
        }))
        .build()
        .map_err(|e| e.to_string())
}
async fn small(
    client: &reqwest::Client,
    url: &str,
    limit: usize,
) -> Result<Option<Vec<u8>>, String> {
    let mut r = client
        .get(url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| "Connexion à GitHub impossible")?;
    if r.status() == 404 {
        return Ok(None);
    }
    if !r.status().is_success() {
        return Err(format!("GitHub : réponse {}", r.status()));
    }
    let mut bytes = vec![];
    while let Some(chunk) = r.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > limit {
            return Err("Réponse de mise à jour trop volumineuse".into());
        }
        bytes.extend_from_slice(&chunk)
    }
    Ok(Some(bytes))
}
fn verified(bytes: &[u8], signature: &[u8], key: &str, current: &str) -> Result<Manifest, String> {
    let key: [u8; 32] = STANDARD
        .decode(key.trim())
        .map_err(|_| "Clé publique invalide")?
        .try_into()
        .map_err(|_| "Clé publique invalide")?;
    let sig = STANDARD
        .decode(
            std::str::from_utf8(signature)
                .map_err(|_| "Signature invalide")?
                .trim(),
        )
        .map_err(|_| "Signature invalide")?;
    VerifyingKey::from_bytes(&key)
        .map_err(|_| "Clé publique invalide")?
        .verify_strict(
            bytes,
            &Signature::from_slice(&sig).map_err(|_| "Signature invalide")?,
        )
        .map_err(|_| "Signature de mise à jour invalide")?;
    let m: Manifest = serde_json::from_slice(bytes).map_err(|_| "Manifeste invalide")?;
    let version = semver::Version::parse(&m.version).map_err(|_| "Version invalide")?;
    if m.format != 1
        || m.target != "windows-x86_64"
        || !version.pre.is_empty()
        || !version.build.is_empty()
        || version <= semver::Version::parse(current).map_err(|_| "Version locale invalide")?
    {
        return Err("Version non applicable".into());
    }
    let expected = format!(
        "https://github.com/extraopaa/mcp-win/releases/download/v{}/mcp-win-v{}-windows-x64.zip",
        m.version, m.version
    );
    if m.url != expected
        || m.size == 0
        || m.size > MAX_ARCHIVE
        || m.sha256.len() != 64
        || !m.sha256.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err("Paquet de mise à jour invalide".into());
    }
    Ok(m)
}
#[tauri::command]
pub fn update_status(state: State<'_, Updater>) -> Value {
    snapshot(&state)
}
#[tauri::command]
pub async fn check_update(force: bool, state: State<'_, Updater>) -> Result<Value, String> {
    {
        let mut s = state.inner.lock().unwrap();
        if s.busy
            || s.pending.is_some()
            || (!force && crate::v1::state::now().saturating_sub(s.last_check) < 21600)
        {
            drop(s);
            return Ok(snapshot(&state));
        }
        s.busy = true;
        s.status = "checking".into();
        s.error = None;
        s.last_check = crate::v1::state::now();
    }
    let result = async {
        let c = client()?;
        let Some(bytes) = small(&c, ENDPOINT, 16384).await? else {
            return Ok(None);
        };
        let sig = small(&c, &format!("{ENDPOINT}.sig"), 512)
            .await?
            .ok_or("Signature de mise à jour absente")?;
        // Authenticate even old manifests; never trust unsigned version metadata.
        match verified(&bytes, &sig, PUBLIC_KEY, env!("CARGO_PKG_VERSION")) {
            Ok(m) => Ok(Some(m.version)),
            Err(e) if e == "Version non applicable" => Ok(None),
            Err(e) => Err(e),
        }
    }
    .await;
    let mut s = state.inner.lock().unwrap();
    s.busy = false;
    match result {
        Ok(version) => {
            s.status = if version.is_some() {
                "available"
            } else {
                "current"
            }
            .into();
            s.version = version;
            s.error = None
        }
        Err(error) => {
            s.status = "error".into();
            s.error = Some(error)
        }
    }
    drop(s);
    Ok(snapshot(&state))
}
#[tauri::command]
pub async fn download_update(state: State<'_, Updater>) -> Result<Value, String> {
    {
        let mut s = state.inner.lock().unwrap();
        if s.pending.is_some() {
            drop(s);
            return Ok(snapshot(&state));
        }
        if s.busy {
            return Err("Mise à jour déjà en cours".into());
        }
        s.busy = true;
        s.status = "downloading".into();
        s.progress = 0;
        s.error = None;
    }
    let result: Result<Pending, String> = async {
        let c = client()?;
        let bytes = small(&c, ENDPOINT, 16384)
            .await?
            .ok_or("Aucune mise à jour publiée")?;
        let signature = small(&c, &format!("{ENDPOINT}.sig"), 512)
            .await?
            .ok_or("Signature absente")?;
        let m = verified(&bytes, &signature, PUBLIC_KEY, env!("CARGO_PKG_VERSION"))?;
        let root = crate::v1::state::root()?.join("updates");
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let used: u64 = walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(Result::ok)
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .sum();
        if used.saturating_add(m.size) > 2 * MAX_ARCHIVE {
            return Err("Cache de mises à jour plein".into());
        }
        let dir = tempfile::Builder::new()
            .prefix("update-")
            .tempdir_in(&root)
            .map_err(|e| e.to_string())?;
        fs::write(dir.path().join("update.json"), bytes).map_err(|e| e.to_string())?;
        fs::write(dir.path().join("update.json.sig"), signature).map_err(|e| e.to_string())?;
        let mut r = c
            .get(&m.url)
            .send()
            .await
            .map_err(|_| "Téléchargement impossible")?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let mut file =
            fs::File::create(dir.path().join("package.zip")).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut received = 0u64;
        while let Some(chunk) = r.chunk().await.map_err(|e| e.to_string())? {
            received += chunk.len() as u64;
            if received > m.size {
                return Err("Taille du paquet incorrecte".into());
            }
            file.write_all(&chunk).map_err(|e| e.to_string())?;
            hash.update(&chunk);
            state.inner.lock().unwrap().progress = received * 100 / m.size;
        }
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        if received != m.size || format!("{:x}", hash.finalize()) != m.sha256.to_lowercase() {
            return Err("Empreinte du paquet incorrecte".into());
        }
        let package = dir.path().join("package.zip");
        tauri::async_runtime::spawn_blocking(move || validate_archive(&package))
            .await
            .map_err(|e| e.to_string())??;
        Ok(Pending {
            directory: dir.keep(),
            manifest: m,
        })
    }
    .await;
    let mut s = state.inner.lock().unwrap();
    s.busy = false;
    match result {
        Ok(p) => {
            s.version = Some(p.manifest.version.clone());
            s.pending = Some(p);
            s.status = "ready".into();
            s.progress = 100
        }
        Err(e) => {
            s.status = "error".into();
            s.error = Some(e)
        }
    }
    drop(s);
    Ok(snapshot(&state))
}
fn owned_path(name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || name.contains(['\\', ':', '\0'])
        || name.starts_with('/')
        || name.ends_with('.')
        || name
            .split('/')
            .any(|c| c.is_empty() || c == "." || c == ".." || c.ends_with(['.', ' ']))
    {
        return Err("Chemin du paquet refusé".into());
    }
    let path = Path::new(name);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Chemin du paquet refusé".into());
    }
    for c in name.split('/') {
        let stem = c.split('.').next().unwrap_or("").to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.as_bytes()[3].is_ascii_digit())
        {
            return Err("Nom Windows réservé".into());
        }
    }
    let first = name.split('/').next().unwrap_or("");
    let root_file = !name.contains('/')
        && (matches!(
            name,
            "MCP Win.exe"
                | "README.md"
                | "CHANGELOG.md"
                | "VALIDATION.md"
                | "LICENSE"
                | "THIRD-PARTY-NOTICES.md"
                | "OCR-MODELS-NOTICE.md"
        ) || name.starts_with("LICENSE-")
            || name.starts_with("NOTICE-"));
    if !root_file
        && !name.starts_with("assets/brand/")
        && !matches!(
            first,
            "engines" | "docs" | "relay" | "third-party-licenses" | "licenses-pdfium"
        )
    {
        return Err("Fichier hors du paquet applicatif".into());
    }
    Ok(path.into())
}
fn validate_archive(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut zip = zip::ZipArchive::new(fs::File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if zip.len() > 12000 {
        return Err("Paquet trop volumineux".into());
    }
    let mut names = HashSet::new();
    let mut paths = vec![];
    let mut size = 0u64;
    for i in 0..zip.len() {
        let f = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().trim_end_matches('/');
        let path = owned_path(name)?;
        if f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
            || !names.insert(name.to_ascii_lowercase())
        {
            return Err("Lien ou doublon dans le paquet".into());
        }
        size = size.checked_add(f.size()).ok_or("Taille invalide")?;
        if size > MAX_EXPANDED || f.size() > 512 * 1024 * 1024 {
            return Err("Paquet décompressé trop volumineux".into());
        }
        if !f.is_dir() {
            paths.push(path)
        }
    }
    if !paths.iter().any(|p| p == Path::new("MCP Win.exe"))
        || !paths.iter().any(|p| p == Path::new("engines/typst.exe"))
        || !paths.iter().any(|p| p == Path::new("engines/pdfium.dll"))
    {
        return Err("Paquet applicatif incomplet".into());
    }
    Ok(paths)
}
#[derive(Serialize, Deserialize)]
struct Plan {
    target: PathBuf,
    executable: String,
    parent: u32,
}
#[tauri::command]
pub fn install_update(
    app: tauri::AppHandle,
    state: State<'_, Updater>,
    desktop: State<'_, crate::desktop::Desktop>,
) -> Result<(), String> {
    desktop
        .check_stopped()
        .map_err(|_| "Arrêtez le serveur avant d’installer la mise à jour")?;
    let s = state.inner.lock().unwrap();
    let p = s
        .pending
        .as_ref()
        .ok_or("Téléchargez d’abord la mise à jour")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let target = exe
        .parent()
        .ok_or("Dossier de l’application absent")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    // Probe write access before exiting; no elevation or downloaded script.
    tempfile::NamedTempFile::new_in(&target)
        .map_err(|_| "Le dossier de l’application n’est pas accessible en écriture")?;
    let plan = Plan {
        target,
        executable: exe
            .file_name()
            .ok_or("Exécutable absent")?
            .to_string_lossy()
            .into(),
        parent: std::process::id(),
    };
    fs::write(
        p.directory.join("plan.json"),
        serde_json::to_vec(&plan).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let helper = p.directory.join("update-helper.exe");
    fs::copy(exe, &helper).map_err(|e| e.to_string())?;
    let mut command = std::process::Command::new(helper);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .arg("--apply-update")
        .arg(&p.directory)
        .spawn()
        .map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}
fn no_links(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for part in relative.components() {
        current.push(part);
        if let Ok(m) = fs::symlink_metadata(&current) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if m.file_attributes() & 0x400 != 0 {
                    return Err("Jonction ou lien de destination refusé".into());
                }
            }
            if m.file_type().is_symlink() {
                return Err("Lien de destination refusé".into());
            }
        }
    }
    Ok(())
}
fn install_files(package: &Path, target: &Path, executable: &str) -> Result<(), String> {
    no_links(target, Path::new(".mcp-update.lock"))?;
    let mut lock_options = fs::OpenOptions::new();
    lock_options.write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        lock_options.share_mode(0);
    }
    let _install_lock = lock_options
        .open(target.join(".mcp-update.lock"))
        .map_err(|_| "Une autre mise à jour est active ou le dossier est verrouillé")?;
    if Path::new(executable).components().count() != 1
        || !executable.to_lowercase().ends_with(".exe")
    {
        return Err("Nom d’exécutable invalide".into());
    }
    let paths = validate_archive(package)?;
    let transaction = tempfile::Builder::new()
        .prefix(".mcp-update-")
        .tempdir_in(target)
        .map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(fs::File::open(package).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let staged = transaction.path().join("new");
    let backup = transaction.path().join("backup");
    let mut targets = vec![];
    for path in paths {
        let dest = if path == Path::new("MCP Win.exe") {
            PathBuf::from(executable)
        } else {
            path.clone()
        };
        no_links(target, &dest)?;
        let out = staged.join(&dest);
        fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
        let name = path.to_string_lossy().replace('\\', "/");
        let mut source = zip.by_name(&name).map_err(|e| e.to_string())?;
        let mut file = fs::File::create(out).map_err(|e| e.to_string())?;
        std::io::copy(&mut source, &mut file).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        targets.push(dest);
    }
    // Executable last; all archive data and destination paths checked first.
    targets.sort_by_key(|p| p == Path::new(executable));
    let mut changed: Vec<(PathBuf, bool)> = vec![];
    let result: Result<(), String> = (|| {
        for path in &targets {
            no_links(target, path)?;
            let dest = target.join(path);
            let old = backup.join(path);
            fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
            let existed = dest.exists();
            if existed {
                if !dest.is_file() {
                    return Err("Destination non fichier".into());
                }
                fs::create_dir_all(old.parent().unwrap()).map_err(|e| e.to_string())?;
                fs::rename(&dest, &old).map_err(|e| e.to_string())?;
            }
            changed.push((path.clone(), existed));
            fs::write(
                transaction.path().join("journal.json"),
                serde_json::to_vec(&changed).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            fs::rename(staged.join(path), dest).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut rollback_errors = vec![];
        for (path, existed) in changed.iter().rev() {
            let dest = target.join(path);
            if dest.exists() {
                if let Err(e) = fs::remove_file(&dest) {
                    rollback_errors.push(e.to_string());
                    continue;
                }
            }
            if *existed {
                if let Err(e) = fs::rename(backup.join(path), dest) {
                    rollback_errors.push(e.to_string())
                }
            }
        }
        if !rollback_errors.is_empty() {
            let recovery = transaction.keep();
            return Err(format!(
                "{error}. Restauration manuelle : {} ({})",
                recovery.display(),
                rollback_errors.join("; ")
            ));
        }
        return Err(format!("Mise à jour annulée, fichiers restaurés : {error}"));
    }
    Ok(())
}
pub fn helper_main() -> bool {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_none_or(|a| a != "--apply-update") {
        return false;
    }
    if args.len() != 3 {
        return true;
    }
    let directory = PathBuf::from(&args[2]);
    let result = (|| -> Result<(), String> {
        let plan: Plan = serde_json::from_slice(
            &fs::read(directory.join("plan.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let bytes = fs::read(directory.join("update.json")).map_err(|e| e.to_string())?;
        let sig = fs::read(directory.join("update.json.sig")).map_err(|e| e.to_string())?;
        let m = verified(&bytes, &sig, PUBLIC_KEY, env!("CARGO_PKG_VERSION"))?;
        let package = directory.join("package.zip");
        let mut f = fs::File::open(&package).map_err(|e| e.to_string())?;
        if f.metadata().map_err(|e| e.to_string())?.len() != m.size {
            return Err("Taille incorrecte".into());
        }
        let mut hash = Sha256::new();
        let mut chunk = [0u8; 65536];
        loop {
            let n = f.read(&mut chunk).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hash.update(&chunk[..n]);
        }
        if format!("{:x}", hash.finalize()) != m.sha256.to_lowercase() {
            return Err("Paquet altéré".into());
        }
        drop(f);
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::{
                Foundation::CloseHandle,
                System::Threading::{OpenProcess, WaitForSingleObject},
            };
            let process = OpenProcess(0x00100000, 0, plan.parent);
            if !process.is_null() {
                let result = WaitForSingleObject(process, 60000);
                CloseHandle(process);
                if result != 0 {
                    return Err("L’application ne s’est pas fermée".into());
                }
            } else if std::io::Error::last_os_error().raw_os_error() != Some(87) {
                return Err("Impossible de vérifier la fermeture de l’application".into());
            }
        }
        install_files(&package, &plan.target, &plan.executable)?;
        let mut launch = std::process::Command::new(plan.target.join(&plan.executable));
        launch.current_dir(&plan.target);
        launch
            .spawn()
            .map_err(|e| format!("Mise à jour installée ; relance manuelle nécessaire : {e}"))?;
        Ok(())
    })();
    let _ = fs::write(
        directory.join("result.json"),
        serde_json::to_vec(&result).unwrap_or_default(),
    );
    if let Err(error) = result {
        let _ = rfd::MessageDialog::new()
            .set_title("MCP Win — Mise à jour")
            .set_description(error)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    fn manifest(version: &str) -> Manifest {
        Manifest {
            format: 1,
            version: version.into(),
            target: "windows-x86_64".into(),
            url: format!(
                "https://github.com/extraopaa/mcp-win/releases/download/v{version}/mcp-win-v{version}-windows-x64.zip"
            ),
            sha256: "a".repeat(64),
            size: 123,
        }
    }
    #[test]
    fn signature_version_and_origin_are_mandatory() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let public = STANDARD.encode(key.verifying_key().as_bytes());
        let m = manifest("1.2.3");
        let bytes = serde_json::to_vec(&m).unwrap();
        let sig = STANDARD.encode(key.sign(&bytes).to_bytes());
        assert!(verified(&bytes, sig.as_bytes(), &public, "1.0.0").is_ok());
        let mut tampered = bytes.clone();
        tampered[0] = b' ';
        assert!(verified(&tampered, sig.as_bytes(), &public, "1.0.0").is_err());
        assert!(verified(&bytes, sig.as_bytes(), &public, "2.0.0").is_err());
        assert!(verified(&bytes, sig.as_bytes(), PUBLIC_KEY, "1.0.0").is_err());
        for version in ["1.2.3-beta.1", "1.2.3+untrusted"] {
            let bytes = serde_json::to_vec(&manifest(version)).unwrap();
            let sig = STANDARD.encode(key.sign(&bytes).to_bytes());
            assert!(verified(&bytes, sig.as_bytes(), &public, "1.0.0").is_err());
        }
        let mut m = m;
        m.url = "https://evil.example/package.zip".into();
        let bytes = serde_json::to_vec(&m).unwrap();
        let sig = STANDARD.encode(key.sign(&bytes).to_bytes());
        assert!(verified(&bytes, sig.as_bytes(), &public, "1.0.0").is_err());
    }
    fn package(path: &Path, extra: Option<&str>) {
        let mut z = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for name in ["engines/typst.exe", "engines/pdfium.dll", "MCP Win.exe"]
            .into_iter()
            .chain(extra)
        {
            z.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b"new").unwrap();
        }
        z.finish().unwrap();
    }
    #[test]
    fn archive_rejects_traversal_reserved_files_and_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "../escape.txt",
            "/absolute.txt",
            "engines/../../escape",
            "settings.json",
            "engines/NUL.txt",
            "engines/file.exe:stream",
            "engines/file. ",
            "ENGINES/typst.exe",
        ] {
            let p = dir.path().join("bad.zip");
            package(&p, Some(name));
            assert!(validate_archive(&p).is_err(), "{name}");
        }
        let path = dir.path().join("good.zip");
        package(&path, None);
        assert_eq!(validate_archive(&path).unwrap().len(), 3);
    }
    #[test]
    fn portable_install_preserves_user_files_and_rolls_back_failure() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("package.zip");
        package(&p, None);
        let target = dir.path().join("app");
        fs::create_dir_all(target.join("engines")).unwrap();
        fs::write(target.join("settings.json"), b"user settings").unwrap();
        fs::write(target.join("renamed.exe"), b"old app").unwrap();
        fs::write(target.join("engines/typst.exe"), b"old engine").unwrap();
        fs::create_dir(target.join("engines/pdfium.dll")).unwrap();
        assert!(install_files(&p, &target, "renamed.exe").is_err());
        assert_eq!(
            fs::read(target.join("engines/typst.exe")).unwrap(),
            b"old engine"
        );
        assert_eq!(fs::read(target.join("renamed.exe")).unwrap(), b"old app");
        fs::remove_dir(target.join("engines/pdfium.dll")).unwrap();
        install_files(&p, &target, "renamed.exe").unwrap();
        assert_eq!(fs::read(target.join("renamed.exe")).unwrap(), b"new");
        assert!(!target.join("MCP Win.exe").exists());
        assert_eq!(
            fs::read(target.join("settings.json")).unwrap(),
            b"user settings"
        );
    }
}
