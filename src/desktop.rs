use crate::{
    library::{self, SharedFolder},
    server::{self, Logs},
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tauri::State;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    pub mode: String,
    pub port: u16,
    pub token: String,
    pub require_auth: bool,
    pub relay: String,
    pub frp_server: String,
    pub frp_port: u16,
    pub frp_token: String,
    pub public_url: String,
    pub frp_ca_pem: String,
    pub frp_tls_name: String,
}
impl Default for Connection {
    fn default() -> Self {
        Self {
            mode: "cloudflare".into(),
            port: 8787,
            token: uuid::Uuid::new_v4().simple().to_string(),
            require_auth: true,
            relay: "https://localtunnel.me".into(),
            frp_server: String::new(),
            frp_port: 7000,
            frp_token: String::new(),
            public_url: String::new(),
            frp_ca_pem: String::new(),
            frp_tls_name: String::new(),
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    security_revision: u32,
    connection: Connection,
    folders: Vec<SharedFolder>,
}
struct Running {
    id: String,
    cancel: CancellationToken,
    address: Option<String>,
    ready: bool,
    active: Arc<AtomicUsize>,
    job: Option<crate::process_job::ProcessJob>,
}
struct Core {
    preferences: Mutex<Preferences>,
    running: Mutex<Option<Running>>,
    logs: Logs,
    directory: PathBuf,
}
pub struct Desktop {
    core: Arc<Core>,
}
impl Desktop {
    pub fn new(directory: PathBuf) -> Self {
        let mut prefs: Preferences = std::fs::read(directory.join("settings.json"))
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default();
        // Retire any credential that may have travelled through an insecure legacy relay.
        let migrate = prefs.security_revision < 1;
        if migrate {
            prefs.connection.token = uuid::Uuid::new_v4().simple().to_string();
            prefs.security_revision = 1;
        }
        // Migrate insecure historical preferences without opening any connection.
        if prefs.connection.mode == "localtunnel" {
            prefs.connection.mode = "local".into();
        }
        prefs.connection.require_auth = true;
        if prefs.connection.token.len() < 32 {
            prefs.connection.token = uuid::Uuid::new_v4().simple().to_string();
        }
        let desktop = Self {
            core: Arc::new(Core {
                preferences: Mutex::new(prefs),
                running: Mutex::new(None),
                logs: Arc::new(Mutex::new(VecDeque::new())),
                directory,
            }),
        };
        if migrate {
            if let Err(_) = desktop.persist() {
                server::log(
                    &desktop.core.logs,
                    "Migration sécurisée en mémoire ; enregistrement des préférences impossible",
                );
            }
            server::log(
                &desktop.core.logs,
                "Sécurité v0.4 : ancien jeton révoqué, authentification obligatoire",
            );
        }
        desktop
    }
    pub fn stop(&self) {
        if let Some(running) = self.core.running.lock().unwrap().take() {
            running.cancel.cancel();
            server::log(&self.core.logs, "Arrêt du MCP et du tunnel demandé");
        }
    }
    fn persist(&self) -> Result<(), String> {
        let data = serde_json::to_vec_pretty(&*self.core.preferences.lock().unwrap())
            .map_err(|e| e.to_string())?;
        let file = self.core.directory.join("settings.json");
        std::fs::write(file, data).map_err(|e| e.to_string())
    }
    fn check_stopped(&self) -> Result<(), String> {
        if self.core.running.lock().unwrap().is_some() {
            Err("Arrêtez le MCP avant de modifier sa configuration".into())
        } else {
            Ok(())
        }
    }
}

#[tauri::command]
pub fn snapshot(state: State<'_, Desktop>) -> Value {
    let prefs = state.core.preferences.lock().unwrap().clone();
    let running = state.core.running.lock().unwrap();
    json!({"connection": prefs.connection, "folders": prefs.folders.iter().map(|f| json!({"id": f.id, "name": f.name(), "path": f.path, "writable":f.writable})).collect::<Vec<_>>(),
        "running": running.is_some(), "ready": running.as_ref().is_some_and(|r| r.ready),
        "address": running.as_ref().and_then(|r| r.address.clone()), "active_connections": running.as_ref().map(|r| r.active.load(Ordering::Relaxed)).unwrap_or(0),
        "logs": state.core.logs.lock().unwrap().clone()})
}
#[tauri::command]
pub async fn add_folder(state: State<'_, Desktop>) -> Result<(), String> {
    state.check_stopped()?;
    let selected = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Choisir un dossier accessible à l'IA")
            .pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Some(path) = selected {
        state.check_stopped()?;
        let path = path.canonicalize().map_err(|e| e.to_string())?;
        let mut prefs = state.core.preferences.lock().unwrap();
        if !prefs.folders.iter().any(|f| f.path == path) {
            prefs.folders.push(SharedFolder {
                writable: false,
                id: uuid::Uuid::new_v4().simple().to_string(),
                path,
            });
        }
        drop(prefs);
        state.persist()?;
    }
    Ok(())
}
#[tauri::command]
pub fn remove_folder(id: String, state: State<'_, Desktop>) -> Result<(), String> {
    state.check_stopped()?;
    state
        .core
        .preferences
        .lock()
        .unwrap()
        .folders
        .retain(|f| f.id != id);
    state.persist()
}
#[tauri::command]
pub fn set_folder_writable(
    id: String,
    writable: bool,
    state: State<'_, Desktop>,
) -> Result<(), String> {
    state.check_stopped()?;
    let mut prefs = state.core.preferences.lock().unwrap();
    let folder = prefs
        .folders
        .iter_mut()
        .find(|f| f.id == id)
        .ok_or("Dossier inconnu")?;
    folder.writable = writable;
    drop(prefs);
    state.persist()
}
#[tauri::command]
pub fn save_settings(connection: Connection, state: State<'_, Desktop>) -> Result<(), String> {
    state.check_stopped()?;
    validate_connection(&connection)?;
    if connection.port == 0 || (connection.require_auth && connection.token.len() < 32) {
        return Err("Port invalide ou jeton trop court (24 caractères minimum)".into());
    }
    if !["cloudflare", "frp", "local"].contains(&connection.mode.as_str()) {
        return Err("Mode de connexion inconnu".into());
    }
    state.core.preferences.lock().unwrap().connection = connection;
    state.persist()
}
#[tauri::command]
pub fn stop_mcp(state: State<'_, Desktop>) {
    state.stop();
}

#[tauri::command]
pub async fn start_mcp(state: State<'_, Desktop>) -> Result<(), String> {
    let prefs = state.core.preferences.lock().unwrap().clone();
    validate_connection(&prefs.connection)?;
    if (prefs.connection.require_auth && prefs.connection.token.len() < 32)
        || prefs.connection.port == 0
    {
        return Err("Port ou jeton MCP invalide".into());
    }
    if prefs.folders.is_empty() {
        return Err("Ajoutez au moins un dossier à partager".into());
    }
    if prefs.folders.iter().any(|f| !f.path.is_dir()) {
        return Err("Un dossier partagé est devenu inaccessible".into());
    }
    let cancel = CancellationToken::new();
    let id = uuid::Uuid::new_v4().to_string();
    let active = Arc::new(AtomicUsize::new(0));
    {
        let mut running = state.core.running.lock().unwrap();
        if running.is_some() {
            return Err("Le serveur est déjà démarré".into());
        }
        *running = Some(Running {
            id: id.clone(),
            cancel: cancel.clone(),
            address: None,
            ready: false,
            active: active.clone(),
            job: None,
        });
    }
    server::log(
        &state.core.logs,
        "Démarrage du MCP et ouverture de la connexion…",
    );
    let core = state.core.clone();
    let result = tokio::select! {
        _ = cancel.cancelled() => Err("Démarrage annulé".into()),
        result = launch(core.clone(), prefs, cancel.clone(), active, id.clone()) => result,
    };
    if let Err(error) = &result {
        cancel.cancel();
        finish(&core, &id);
        let _ = std::fs::remove_file(core.directory.join(format!("frpc-{id}.json")));
        let _ = std::fs::remove_file(core.directory.join(format!("frpc-{id}.ca.pem")));
        server::log(&core.logs, error);
    }
    result
}
fn finish(core: &Core, id: &str) {
    let mut running = core.running.lock().unwrap();
    if running.as_ref().is_some_and(|r| r.id == id) {
        running.take();
    }
}

async fn launch(
    core: Arc<Core>,
    prefs: Preferences,
    cancel: CancellationToken,
    _active: Arc<AtomicUsize>,
    id: String,
) -> Result<(), String> {
    let c = prefs.connection;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", c.port))
        .await
        .map_err(|e| format!("Port local indisponible : {e}"))?;
    validate_connection(&c)?;
    let mut cloudflare = None;
    let address = match c.mode.as_str() {
        "cloudflare" => {
            let tunnel = crate::cloudflare::start(c.port, &cancel).await?;
            let address = format!("{}/mcp", tunnel.url);
            cloudflare = Some(tunnel);
            address
        }
        "frp" => {
            let url = reqwest::Url::parse(&c.public_url)
                .map_err(|_| "Renseignez l'URL HTTPS de votre relais personnel")?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || url.username() != ""
                || url.password().is_some()
            {
                return Err("Le relais personnel doit avoir une URL publique HTTPS".into());
            }
            if c.frp_server.trim().is_empty() || c.frp_token.is_empty() {
                return Err("Adresse et jeton frps requis".into());
            }
            format!(
                "{}/mcp",
                c.public_url.trim_end_matches('/').trim_end_matches("/mcp")
            )
        }
        "local" => format!("http://127.0.0.1:{}/mcp", c.port),
        _ => return Err("Mode de connexion inconnu".into()),
    };
    let url = reqwest::Url::parse(&address).map_err(|e| e.to_string())?;
    let public_host = if c.mode == "local" {
        None
    } else {
        Some(match url.port() {
            Some(port) => format!("{}:{port}", url.host_str().unwrap()),
            None => url.host_str().unwrap().to_owned(),
        })
    };
    let mut frp = if c.mode == "frp" {
        Some(spawn_frp(&core, &c, &id, url.host_str().unwrap()).await?)
    } else {
        None
    };
    {
        let mut running = core.running.lock().unwrap();
        let current = running
            .as_mut()
            .filter(|r| r.id == id)
            .ok_or("Démarrage annulé")?;
        current.address = Some(address);
        current.ready = true;
        if let Some(child) = &frp {
            current.job = Some(crate::process_job::ProcessJob::attach(child)?);
        }
    }
    let server_cancel = cancel.clone();
    let logs = core.logs.clone();
    tauri::async_runtime::spawn(async move {
        let server_task = tokio::spawn(server::serve(
            listener,
            prefs.folders,
            c.require_auth.then_some(c.token),
            server_cancel.clone(),
            logs.clone(),
            public_host,
        ));
        tokio::select! {
            _ = cancel.cancelled() => {},
            result = async { if let Some(child) = &mut frp { child.wait().await.map(|s| format!("Le relais personnel s'est arrêté : {s}")) } else { std::future::pending().await } } => { if let Ok(message) = result { server::log(&logs, message); } },
            result = async { if let Some(tunnel) = &mut cloudflare { tunnel.child.wait().await.map(|s| format!("Le tunnel Cloudflare s'est arrêté : {s}")) } else { std::future::pending().await } } => { if let Ok(message) = result { server::log(&logs, message); } },
            _ = async { while !server_task.is_finished() { tokio::time::sleep(std::time::Duration::from_millis(200)).await; } } => { server::log(&logs, "Le serveur MCP s'est arrêté"); },
        }
        cancel.cancel();
        if let Some(mut child) = frp {
            let _ = child.kill().await;
        }
        if let Some(mut tunnel) = cloudflare {
            let _ = tunnel.child.kill().await;
        }
        let _ = server_task.await;
        let _ = std::fs::remove_file(core.directory.join(format!("frpc-{id}.json")));
        let _ = std::fs::remove_file(core.directory.join(format!("frpc-{id}.ca.pem")));
        finish(&core, &id);
    });
    Ok(())
}

fn validate_connection(c: &Connection) -> Result<(), String> {
    if !["local", "frp", "cloudflare"].contains(&c.mode.as_str()) {
        return Err("Tunnel localtunnel désactivé : transport non chiffré. Utilisez un relais frp avec certificat vérifié.".into());
    }
    if !c.require_auth
        || c.token.len() < 32
        || c.token.len() > 256
        || !c
            .token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(
            "Un jeton MCP de 32 caractères minimum est obligatoire (lettres, chiffres, tirets)."
                .into(),
        );
    }
    if c.port == 0 {
        return Err("Port invalide".into());
    }
    if c.mode == "frp" {
        if c.frp_ca_pem.len() > 65536
            || !c.frp_ca_pem.contains("-----BEGIN CERTIFICATE-----")
            || c.frp_tls_name.trim().is_empty()
            || c.frp_token.len() < 32
        {
            return Err("Relais sécurisé : certificat CA PEM, nom TLS du certificat et jeton frp de 32 caractères requis.".into());
        }
        let url = reqwest::Url::parse(&c.public_url).map_err(|_| "URL HTTPS invalide")?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !["/", "/mcp", ""].contains(&url.path())
        {
            return Err(
                "Utilisez une URL HTTPS sans identifiants, paramètres ni chemin autre que /mcp."
                    .into(),
            );
        }
    }
    Ok(())
}

async fn spawn_frp(
    core: &Core,
    c: &Connection,
    id: &str,
    domain: &str,
) -> Result<tokio::process::Child, String> {
    validate_connection(c)?;
    let bytes = include_bytes!("../vendor/frp/frp_0.71.1_windows_amd64/frpc.exe");
    let binary = core.directory.join("frpc-0.71.1.exe");
    if std::fs::read(&binary).ok().as_deref() != Some(bytes.as_slice()) {
        std::fs::write(&binary, bytes).map_err(|e| e.to_string())?;
    }
    let ca_path = core.directory.join(format!("frpc-{id}.ca.pem"));
    std::fs::write(&ca_path, &c.frp_ca_pem).map_err(|e| e.to_string())?;
    let config = json!({"serverAddr": c.frp_server.trim(), "serverPort": c.frp_port, "auth": {"method":"token", "token":c.frp_token},
        "transport":{"tls":{"enable":true,"trustedCaFile":ca_path,"serverName":c.frp_tls_name.trim()}}, "loginFailExit":true, "log":{"to":"console","level":"info","disablePrintColor":true},
        "proxies":[{"name":format!("mcp-win-{id}"),"type":"http","localIP":"127.0.0.1","localPort":c.port,"customDomains":[domain],"hostHeaderRewrite":format!("127.0.0.1:{}", c.port)}]});
    let path = core.directory.join(format!("frpc-{id}.json"));
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).map_err(|e| e.to_string())?;
    let mut command = tokio::process::Command::new(binary);
    command
        .args(["-c"])
        .arg(path)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command
        .spawn()
        .map_err(|e| format!("Impossible de lancer frpc : {e}"))?;
    use tokio::io::AsyncBufReadExt;
    let stdout = child.stdout.take().ok_or("Sortie frpc indisponible")?;
    let mut lines = tokio::io::BufReader::new(stdout).lines();
    let expected = format!("[mcp-win-{id}] start proxy success");
    let ready = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        while let Some(line) = lines
            .next_line()
            .await
            .map_err(|_| "Lecture frpc impossible")?
        {
            if line.contains(&expected) {
                return Ok::<_, String>(());
            }
        }
        Err(
            "Connexion frp refusée : vérifier le certificat, le nom TLS, le jeton et le relais."
                .into(),
        )
    })
    .await;
    match ready {
        Ok(Ok(())) => {}
        other => {
            let _ = child.kill().await;
            return Err(match other {
                Ok(Err(e)) => e,
                _ => "Le relais n’a pas confirmé l’ouverture sécurisée du proxy en 20 secondes"
                    .into(),
            });
        }
    }
    // Drain trusted sidecar logs without exposing relay diagnostics or secrets to the UI.
    tokio::spawn(async move { while lines.next_line().await.ok().flatten().is_some() {} });
    server::log(&core.logs, "Client frpc démarré (relais personnel)");
    Ok(child)
}

#[tauri::command]
pub async fn list_history(folder: String, state: State<'_, Desktop>) -> Result<Value, String> {
    let folders = state.core.preferences.lock().unwrap().folders.clone();
    tauri::async_runtime::spawn_blocking(move || crate::history::list(&folders, &folder, None))
        .await
        .map_err(|_| "Lecture de l’historique interrompue")?
}
#[tauri::command]
pub async fn restore_version(
    folder: String,
    version: String,
    state: State<'_, Desktop>,
) -> Result<Value, String> {
    state.check_stopped()?;
    let mut folders = state.core.preferences.lock().unwrap().folders.clone();
    // Explicit local restoration is owned by the user; remote writable preferences stay unchanged.
    crate::library::select_folder(&folders, &folder)?;
    let selected_id = crate::library::select_folder(&folders, &folder)?.id.clone();
    if let Some(selected) = folders.iter_mut().find(|f| f.id == selected_id) {
        selected.writable = true;
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::history::restore_local(&folders, &folder, &version)
    })
    .await
    .map_err(|_| "Restauration interrompue")??;
    server::log(&state.core.logs, "Version précédente restaurée localement");
    Ok(result)
}

#[tauri::command]
pub async fn preview_file(folder: String, state: State<'_, Desktop>) -> Result<Value, String> {
    let shared = state
        .core
        .preferences
        .lock()
        .unwrap()
        .folders
        .iter()
        .find(|f| f.id == folder)
        .cloned()
        .ok_or("Dossier inconnu")?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_directory(&shared.path)
            .set_title("Aperçu de lecture")
            .pick_file()
        else {
            return Ok(Value::Null);
        };
        let root = shared.path.canonicalize().map_err(|e| e.to_string())?;
        let path = path.canonicalize().map_err(|e| e.to_string())?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "Choisissez un fichier du dossier partagé")?
            .to_string_lossy()
            .into_owned();
        let mut value = library::read(
            std::slice::from_ref(&shared),
            &shared.id,
            &relative,
            1,
            0,
            12000,
        )?;
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
        {
            let png = library::render_svg(std::slice::from_ref(&shared), &shared.id, &relative)?;
            value["png"] = json!(format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(png)
            ));
        }
        Ok(value)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insecure_settings_are_rejected_and_migrated() {
        let mut c = Connection::default();
        assert!(validate_connection(&c).is_ok());
        c.mode = "localtunnel".into();
        assert!(validate_connection(&c).is_err());
        c.mode = "local".into();
        c.require_auth = false;
        assert!(validate_connection(&c).is_err());
        c.require_auth = true;
        c.mode = "frp".into();
        assert!(validate_connection(&c).is_err());
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("settings.json"),
            br#"{"connection":{"mode":"localtunnel","require_auth":false,"token":"old"}}"#,
        )
        .unwrap();
        let desktop = Desktop::new(temp.path().into());
        let prefs = desktop.core.preferences.lock().unwrap();
        assert_eq!(prefs.connection.mode, "local");
        assert!(prefs.connection.require_auth && prefs.connection.token.len() >= 32);
    }

    #[tokio::test]
    async fn embedded_frpc_routes_to_local_backend_and_job_closes_it() {
        let temp = tempfile::tempdir().unwrap();
        let control_socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let vhost_socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let control = control_socket.local_addr().unwrap().port();
        let vhost = vhost_socket.local_addr().unwrap().port();
        let config = temp.path().join("frps.json");
        std::fs::write(&config, serde_json::to_vec(&json!({"bindAddr":"127.0.0.1","bindPort":control,"proxyBindAddr":"127.0.0.1","vhostHTTPPort":vhost,"auth":{"method":"token","token":"test-frp-token-long-enough-for-security"},"transport":{"tls":{"force":true,"certFile":concat!(env!("CARGO_MANIFEST_DIR"),"/tests/fixtures/relay-cert.pem"),"keyFile":concat!(env!("CARGO_MANIFEST_DIR"),"/tests/fixtures/relay-key.pem")}}})).unwrap()).unwrap();
        let mut relay_command = tokio::process::Command::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/vendor/frp/frp_0.71.1_windows_amd64/frps.exe"
        ));
        relay_command
            .arg("-c")
            .arg(config)
            .kill_on_drop(true)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        relay_command.creation_flags(0x08000000);
        drop((control_socket, vhost_socket));
        let mut relay = relay_command.spawn().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                if let Some(status) = relay.try_wait().unwrap() {
                    panic!("Le relais de test s'est arrêté : {status}");
                }
                if tokio::net::TcpStream::connect(("127.0.0.1", control))
                    .await
                    .is_ok()
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("Le relais de test n'a pas ouvert son port");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let cancel = CancellationToken::new();
        let shutdown = cancel.clone();
        let backend = tokio::spawn(async move {
            axum::serve(
                listener,
                axum::Router::new()
                    .route("/mcp", axum::routing::get(|| async { "FRP-ROUNDTRIP-OK" })),
            )
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            .unwrap();
        });
        let core = Core {
            preferences: Mutex::new(Preferences::default()),
            running: Mutex::new(None),
            logs: Arc::new(Mutex::new(VecDeque::new())),
            directory: temp.path().into(),
        };
        // Controlled wire tap on the frpc -> frps segment: payload must not be plaintext.
        let tap = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tap_port = tap.local_addr().unwrap().port();
        let wire = Arc::new(Mutex::new(Vec::<u8>::new()));
        let tap_cancel = CancellationToken::new();
        let shutdown_tap = tap_cancel.clone();
        let observed = wire.clone();
        let tap_task = tokio::spawn(async move {
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                let incoming = tokio::select! { _ = shutdown_tap.cancelled() => break, incoming = tap.accept() => incoming.unwrap().0 };
                let outgoing = tokio::net::TcpStream::connect(("127.0.0.1", control))
                    .await
                    .unwrap();
                let observed = observed.clone();
                let stop = shutdown_tap.clone();
                tasks.spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let (ir, iw) = incoming.into_split();
                    let (or, ow) = outgoing.into_split();
                    let capture = |mut reader: tokio::net::tcp::OwnedReadHalf, mut writer: tokio::net::tcp::OwnedWriteHalf, bytes: Arc<Mutex<Vec<u8>>>| async move {
                        let mut buf = [0u8;8192];
                        loop { let n = reader.read(&mut buf).await?; if n == 0 { break; }
                            { let mut v = bytes.lock().unwrap(); if v.len() < 1024*1024 { v.extend_from_slice(&buf[..n]); } }
                            writer.write_all(&buf[..n]).await?;
                        }
                        Ok::<_, std::io::Error>(())
                    };
                    tokio::select! { _ = stop.cancelled() => {}, _ = async { tokio::try_join!(capture(ir, ow, observed.clone()), capture(or, iw, observed)) } => {} }
                });
            }
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
        });
        let connection = Connection {
            port,
            frp_server: "127.0.0.1".into(),
            frp_port: tap_port,
            mode: "frp".into(),
            frp_token: "test-frp-token-long-enough-for-security".into(),
            public_url: "https://mcp.test.example".into(),
            frp_ca_pem: include_str!("../tests/fixtures/relay-cert.pem").into(),
            frp_tls_name: "localhost".into(),
            ..Default::default()
        };
        // Active MITM simulation: same reachable endpoint, but a forged TLS identity.
        let mut impostor = connection.clone();
        impostor.frp_tls_name = "attacker.invalid".into();
        assert!(
            spawn_frp(&core, &impostor, "bad-name", "mcp.test.example")
                .await
                .is_err()
        );
        impostor = connection.clone();
        impostor.frp_ca_pem = include_str!("../tests/fixtures/untrusted-cert.pem").into();
        assert!(
            spawn_frp(&core, &impostor, "bad-ca", "mcp.test.example")
                .await
                .is_err()
        );
        let mut child = spawn_frp(&core, &connection, "test", "mcp.test.example")
            .await
            .unwrap();
        let job = crate::process_job::ProcessJob::attach(&child).unwrap();
        let response = reqwest::Client::new()
            .get(format!("http://127.0.0.1:{vhost}/mcp"))
            .header("Host", "mcp.test.example")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        drop(job);
        tokio::time::timeout(std::time::Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap();
        cancel.cancel();
        backend.await.unwrap();
        let _ = relay.kill().await;
        tap_cancel.cancel();
        tap_task.await.unwrap();
        let captured = wire.lock().unwrap();
        assert!(!captured.is_empty());
        for secret in [
            b"FRP-ROUNDTRIP-OK".as_slice(),
            b"GET /mcp",
            b"test-frp-token-long-enough-for-security",
        ] {
            assert!(
                !captured.windows(secret.len()).any(|w| w == secret),
                "Plaintext leaked into tunnel"
            );
        }
        assert_eq!(response, "FRP-ROUNDTRIP-OK");
    }
}
