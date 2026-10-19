use crate::server::{self, Logs};
use serde::Deserialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{io::copy_bidirectional, net::TcpStream};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize)]
struct Allocation {
    url: String,
    port: u16,
    ip: Option<String>,
    max_conn_count: Option<usize>,
}
pub struct Tunnel {
    pub url: String,
    remote_host: String,
    remote_port: u16,
    count: usize,
    first: TcpStream,
}

pub async fn allocate(relay: &str) -> Result<Tunnel, String> {
    let relay = reqwest::Url::parse(relay).map_err(|_| "Adresse du relais invalide")?;
    if relay.scheme() != "https"
        || relay.host_str().is_none()
        || !relay.username().is_empty()
        || relay.password().is_some()
    {
        return Err("Le relais localtunnel doit utiliser une adresse HTTPS".into());
    }
    let endpoint = relay.join("/?new").map_err(|e| e.to_string())?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(endpoint)
        .header("User-Agent", "MCP-Win/0.1")
        .send()
        .await
        .map_err(|e| format!("Relais inaccessible : {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Le relais a répondu HTTP {}", response.status()));
    }
    let allocation: Allocation = response
        .json()
        .await
        .map_err(|_| "Réponse du relais non reconnue")?;
    let url = reqwest::Url::parse(&allocation.url).map_err(|_| "URL publique invalide")?;
    if url.scheme() != "https" || url.host_str().is_none() || allocation.port == 0 {
        return Err("Réponse du relais invalide".into());
    }
    let remote_host = allocation
        .ip
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| relay.host_str().unwrap().to_owned());
    let first = connect(&remote_host, allocation.port).await?;
    Ok(Tunnel {
        url: allocation.url.trim_end_matches('/').to_owned(),
        remote_host,
        remote_port: allocation.port,
        count: allocation.max_conn_count.unwrap_or(1).clamp(1, 10),
        first,
    })
}

async fn connect(host: &str, port: u16) -> Result<TcpStream, String> {
    tokio::time::timeout(Duration::from_secs(10), TcpStream::connect((host, port)))
        .await
        .map_err(|_| "Le port du relais ne répond pas (délai dépassé)".to_owned())?
        .map_err(|e| format!("Connexion au relais impossible : {e}"))
}

pub async fn run(
    tunnel: Tunnel,
    local_port: u16,
    cancel: CancellationToken,
    active: Arc<AtomicUsize>,
    logs: Logs,
) {
    server::log(
        &logs,
        "Tunnel public établi ; reconnexion automatique activée",
    );
    let mut tasks = tokio::task::JoinSet::new();
    let mut first = Some(tunnel.first);
    for _ in 0..tunnel.count {
        let host = tunnel.remote_host.clone();
        let port = tunnel.remote_port;
        let cancel = cancel.clone();
        let active = active.clone();
        let initial = first.take();
        tasks.spawn(async move {
            let mut initial = initial;
            loop {
                let remote = tokio::select! {
                    _ = cancel.cancelled() => break,
                    value = async { if let Some(stream) = initial.take() { Ok(stream) } else { connect(&host, port).await } } => value,
                };
                if let Ok(mut remote) = remote {
                    if let Ok(mut local) = TcpStream::connect(("127.0.0.1", local_port)).await {
                        active.fetch_add(1, Ordering::Relaxed);
                        tokio::select! { _ = cancel.cancelled() => {}, _ = copy_bidirectional(&mut remote, &mut local) => {} }
                        active.fetch_sub(1, Ordering::Relaxed);
                    }
                }
                tokio::select! { _ = cancel.cancelled() => break, _ = tokio::time::sleep(Duration::from_millis(600)) => {} }
            }
        });
    }
    while tasks.join_next().await.is_some() {}
    server::log(&logs, "Tunnel public fermé");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    #[ignore = "nécessite le relais public localtunnel et Internet"]
    async fn public_mcp_smoke() {
        use serde_json::json;
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test.txt"), "MCP-WIN-PUBLIC-SMOKE").unwrap();
        let allocated = allocate("https://localtunnel.me")
            .await
            .expect("allocation du tunnel public");
        let address = format!("{}/mcp", allocated.url);
        let host = reqwest::Url::parse(&address)
            .unwrap()
            .host_str()
            .unwrap()
            .to_owned();
        println!("Test via le relais public : {address}");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let cancel = CancellationToken::new();
        let logs = Arc::new(std::sync::Mutex::new(Default::default()));
        let token = uuid::Uuid::new_v4().to_string();
        let server = tokio::spawn(server::serve(
            listener,
            vec![crate::library::SharedFolder {
                id: "test".into(),
                path: temp.path().into(),
            }],
            Some(token.clone()),
            cancel.clone(),
            logs.clone(),
            Some(host),
        ));
        let tunnel = tokio::spawn(run(
            allocated,
            port,
            cancel.clone(),
            Arc::new(AtomicUsize::new(0)),
            logs,
        ));
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        let response = client.post(&address).bearer_auth(&token).header("bypass-tunnel-reminder", "true")
            .header("Accept", "application/json, text/event-stream").header("MCP-Protocol-Version", "2025-03-26")
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read_file","arguments":{"folder":"test","path":"test.txt"}}})).send().await;
        let outcome = match response {
            Ok(response) => response.text().await.map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        cancel.cancel();
        let _ = server.await;
        let _ = tunnel.await;
        let body = outcome.unwrap();
        assert!(
            body.contains("MCP-WIN-PUBLIC-SMOKE"),
            "Réponse du relais : {body}"
        );
    }
    #[tokio::test]
    async fn forwards_bytes_both_ways_and_cancels() {
        let local = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_port = local.local_addr().unwrap().port();
        let remote = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let remote_port = remote.local_addr().unwrap().port();
        let first = TcpStream::connect(("127.0.0.1", remote_port))
            .await
            .unwrap();
        let tunnel = Tunnel {
            url: "https://test.example".into(),
            remote_host: "127.0.0.1".into(),
            remote_port,
            count: 1,
            first,
        };
        let cancel = CancellationToken::new();
        let task = tokio::spawn(run(
            tunnel,
            local_port,
            cancel.clone(),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(std::sync::Mutex::new(Default::default())),
        ));
        let (mut public, _) = remote.accept().await.unwrap();
        let (mut backend, _) = local.accept().await.unwrap();
        public.write_all(b"request").await.unwrap();
        let mut buf = [0; 7];
        backend.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"request");
        backend.write_all(b"reply").await.unwrap();
        let mut buf = [0; 5];
        public.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"reply");
        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
    }
}
