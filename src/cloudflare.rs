use crate::{advanced, process_job::ProcessJob};
use tokio::io::AsyncBufReadExt;
use tokio_util::sync::CancellationToken;

pub struct Tunnel {
    pub child: tokio::process::Child,
    pub url: String,
    _job: ProcessJob,
    _config: tempfile::TempDir,
}

fn public_url(line: &str) -> Option<String> {
    let pattern = regex::Regex::new(r"https://[a-z0-9-]+\.trycloudflare\.com").ok()?;
    let matched = pattern.find(line)?;
    let next = line[matched.end()..].chars().next();
    if next.is_some_and(|c| !c.is_whitespace() && !['|', '"', '\''].contains(&c)) {
        return None;
    }
    let found = matched.as_str();
    let url = reqwest::Url::parse(found).ok()?;
    (url.scheme() == "https"
        && url.host_str()?.ends_with(".trycloudflare.com")
        && url.path() == "/")
        .then(|| found.to_owned())
}

pub async fn start(port: u16, cancel: &CancellationToken) -> Result<Tunnel, String> {
    let binary = advanced::engine_path("cloudflared.exe")?;
    // Verify the distributed executable before running it, not just at download time.
    let bytes = std::fs::read(&binary).map_err(|_| "Client Cloudflare illisible")?;
    if crate::writing::hash(&bytes)
        != "4d2490e74082c5d48dbfedaf92576fa5bd1702379ab88eb7faa0cfbe8dc891b6"
    {
        return Err("Empreinte cloudflared incorrecte : démarrage refusé".into());
    }
    let config = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = config.path().join("config.yml");
    std::fs::write(&path, "{}\n").map_err(|e| e.to_string())?;
    let mut cmd = tokio::process::Command::new(binary);
    cmd.arg("tunnel")
        .arg("--config")
        .arg(&path)
        .args([
            "--no-autoupdate",
            "--management-diagnostics=false",
            "--url",
            &format!("http://127.0.0.1:{port}"),
            "--http-host-header",
            &format!("127.0.0.1:{port}"),
            "--protocol",
            "auto",
            "--metrics",
            "127.0.0.1:0",
            "--loglevel",
            "info",
        ])
        .current_dir(config.path())
        .env_clear()
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    // Prevent inherited TUNNEL_*, proxy and user config variables from changing routing or TLS.
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    cmd.env("TEMP", config.path()).env("TMP", config.path());
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Démarrage cloudflared impossible : {e}"))?;
    let job = ProcessJob::attach(&child)?;
    let stderr = child
        .stderr
        .take()
        .ok_or("Sortie cloudflared indisponible")?;
    let mut lines = tokio::io::BufReader::new(stderr).lines();
    let ready = async {
        let mut url = None;
        let mut registered = false;
        while let Some(line) = lines
            .next_line()
            .await
            .map_err(|_| "Lecture cloudflared impossible")?
        {
            if let Some(value) = public_url(&line) {
                url = Some(value);
            }
            if line.contains("Registered tunnel connection") {
                registered = true;
            }
            if registered {
                if let Some(url) = url.take() {
                    return Ok::<_, String>(url);
                }
            }
        }
        Err("Cloudflare a refusé la connexion au tunnel".into())
    };
    let url = tokio::select! {
        _ = cancel.cancelled() => Err("Ouverture Cloudflare annulée".into()),
        result = tokio::time::timeout(std::time::Duration::from_secs(60), ready) => result.unwrap_or_else(|_| Err("Cloudflare : aucune connexion chiffrée confirmée en 60 secondes".into())),
    };
    let url = match url {
        Ok(url) => url,
        Err(e) => {
            let _ = child.kill().await;
            return Err(e);
        }
    };
    tokio::spawn(async move { while lines.next_line().await.ok().flatten().is_some() {} });
    Ok(Tunnel {
        child,
        url,
        _job: job,
        _config: config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_only_expected_https_hostnames() {
        assert_eq!(
            public_url("| https://test-safe-bridge.trycloudflare.com |"),
            Some("https://test-safe-bridge.trycloudflare.com".into())
        );
        assert!(public_url("http://test.trycloudflare.com").is_none());
        assert!(public_url("https://attacker.example").is_none());
        assert!(public_url("https://good.trycloudflare.com.attacker.example").is_none());
        assert!(public_url("https://good.trycloudflare.com:8443").is_none());
        assert!(public_url("https://trycloudflare.com").is_none());
    }
    #[tokio::test]
    #[ignore = "Internet Cloudflare ; données synthétiques uniquement"]
    async fn public_tunnel_auth_and_read() {
        use crate::{library::SharedFolder, server};
        use std::sync::{Arc, Mutex};
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test.txt"), "CLOUDFLARE-SYNTHETIC-READ-OK").unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let cancel = CancellationToken::new();
        let mut tunnel = start(port, &cancel).await.unwrap();
        let url = format!("{}/mcp", tunnel.url);
        let host = reqwest::Url::parse(&url)
            .unwrap()
            .host_str()
            .unwrap()
            .to_owned();
        let token = uuid::Uuid::new_v4().simple().to_string();
        let server = tokio::spawn(server::serve(
            listener,
            vec![SharedFolder {
                id: "synthetic".into(),
                path: temp.path().into(),
                writable: false,
            }],
            Some(token.clone()),
            cancel.clone(),
            Arc::new(Mutex::new(Default::default())),
            Some(host),
        ));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap();
        let outcome=async {
            let mut reachable=false;
            for _ in 0..20 {
                let response=client.post(&url).body("{}").send().await?;
                if response.status()==401 {reachable=true;break;}
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            assert!(reachable,"Public endpoint must reject anonymous requests");
            let denied=client.post(&url).bearer_auth("wrong").body("{}").send().await?;
            assert_eq!(denied.status(),401);
            let response=client.post(&url).bearer_auth(&token).header("Accept","application/json, text/event-stream").header("MCP-Protocol-Version","2025-03-26")
                .json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read_file","arguments":{"path":"test.txt"}}})).send().await?;
            assert_eq!(response.status(),200);
            assert!(response.text().await?.contains("CLOUDFLARE-SYNTHETIC-READ-OK"));
            Ok::<_,reqwest::Error>(())
        }.await;
        cancel.cancel();
        let _ = tunnel.child.kill().await;
        server.await.unwrap().unwrap();
        outcome.unwrap();
    }
}
