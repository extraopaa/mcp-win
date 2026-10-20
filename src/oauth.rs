//! OAuth authorization-code + S256 PKCE. Consent is granted only in the desktop UI.
use axum::{
    Form, Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
use subtle::ConstantTimeEq;
type Shared = Arc<Mutex<Session>>;
static CURRENT: OnceLock<Mutex<Option<Shared>>> = OnceLock::new();
fn current() -> &'static Mutex<Option<Shared>> {
    CURRENT.get_or_init(|| Mutex::new(None))
}
fn now() -> u64 {
    crate::v1::state::now()
}
fn random() -> String {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).expect("Windows RNG unavailable");
    URL_SAFE_NO_PAD.encode(b)
}
fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
#[derive(Clone, Serialize, Deserialize)]
struct Client {
    id: String,
    name: String,
    redirects: Vec<String>,
    secret: Option<String>,
    created: u64,
    approved: bool,
}
struct Request {
    client: String,
    redirect: String,
    challenge: String,
    state: String,
    ticket: String,
    expires: u64,
    approved: Option<bool>,
}
struct Code {
    client: String,
    redirect: String,
    challenge: String,
    expires: u64,
}
struct Token {
    client: String,
    family: String,
    expires: u64,
    refresh: bool,
}
pub struct Session {
    pub active: bool,
    issuer: String,
    resource: String,
    clients: HashMap<String, Client>,
    requests: HashMap<String, Request>,
    codes: HashMap<String, Code>,
    tokens: HashMap<String, Token>,
    used: HashMap<String, (String, String, u64)>,
}
pub fn start(issuer: String) -> Shared {
    let clients = crate::v1::state::root()
        .ok()
        .and_then(|p| std::fs::read(p.join("oauth-clients.json")).ok())
        .filter(|b| b.len() < 2 * 1024 * 1024)
        .and_then(|b| serde_json::from_slice::<Vec<Client>>(&b).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.approved)
        .take(100)
        .map(|c| (c.id.clone(), c))
        .collect();
    let session = Arc::new(Mutex::new(Session {
        active: true,
        resource: format!("{issuer}/mcp"),
        issuer,
        clients,
        requests: HashMap::new(),
        codes: HashMap::new(),
        tokens: HashMap::new(),
        used: HashMap::new(),
    }));
    *current().lock().unwrap() = Some(session.clone());
    session
}
pub fn stop() {
    if let Some(shared) = current().lock().unwrap().take() {
        let mut s = shared.lock().unwrap();
        s.active = false;
        s.tokens.clear();
        s.codes.clear();
        s.requests.clear();
    }
}
pub fn finish(shared: &Shared) {
    {
        let mut s = shared.lock().unwrap();
        s.active = false;
        s.tokens.clear();
        s.codes.clear();
        s.requests.clear();
    }
    let mut active = current().lock().unwrap();
    if active.as_ref().is_some_and(|s| Arc::ptr_eq(s, shared)) {
        active.take();
    }
}
impl Session {
    fn prune(&mut self) {
        let t = now();
        self.requests.retain(|_, r| r.expires > t);
        self.codes.retain(|_, c| c.expires > t);
        self.tokens.retain(|_, v| v.expires > t);
        self.used.retain(|_, (_, _, e)| *e > t);
        self.clients
            .retain(|_, c| c.approved || c.created + 600 > t);
    }
    fn persist(&self) -> Result<(), String> {
        crate::v1::state::write_json(
            &crate::v1::state::root()?.join("oauth-clients.json"),
            &self
                .clients
                .values()
                .filter(|c| c.approved)
                .collect::<Vec<_>>(),
        )
    }
    fn revoke(&mut self, family: &str) {
        self.tokens.retain(|_, t| t.family != family);
    }
    fn issue(&mut self, client: String, family: String) -> Value {
        let access = random();
        let refresh = random();
        self.tokens.insert(
            hash(&access),
            Token {
                client: client.clone(),
                family: family.clone(),
                expires: now() + 3600,
                refresh: false,
            },
        );
        self.tokens.insert(
            hash(&refresh),
            Token {
                client,
                family,
                expires: now() + 7 * 86400,
                refresh: true,
            },
        );
        json!({"access_token":access,"token_type":"Bearer","expires_in":3600,"refresh_token":refresh,"scope":"mcp"})
    }
    pub fn valid(&mut self, token: &str) -> bool {
        self.prune();
        self.active && self.tokens.get(&hash(token)).is_some_and(|t| !t.refresh)
    }
}
fn error(code: &str, status: StatusCode) -> Response {
    (status, Json(json!({"error":code}))).into_response()
}
fn safe_redirect(value: &str) -> bool {
    let Ok(u) = reqwest::Url::parse(value) else {
        return false;
    };
    value.len() <= 2048
        && u.fragment().is_none()
        && u.username().is_empty()
        && u.password().is_none()
        && u.host_str().is_some()
        && (u.scheme() == "https"
            || u.scheme() == "http"
                && matches!(u.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")))
        && !u
            .query_pairs()
            .any(|(k, _)| matches!(k.as_ref(), "code" | "state" | "error"))
}
#[derive(Deserialize)]
struct Registration {
    client_name: Option<String>,
    redirect_uris: Vec<String>,
    token_endpoint_auth_method: Option<String>,
    grant_types: Option<Vec<String>>,
    response_types: Option<Vec<String>>,
}
async fn register(State(shared): State<Shared>, Json(r): Json<Registration>) -> Response {
    let method = r
        .token_endpoint_auth_method
        .as_deref()
        .unwrap_or("client_secret_basic");
    if !["none", "client_secret_post", "client_secret_basic"].contains(&method)
        || r.redirect_uris.is_empty()
        || r.redirect_uris.len() > 8
        || r.redirect_uris.iter().any(|u| !safe_redirect(u))
        || r.grant_types.is_some_and(|g| {
            g.iter()
                .any(|s| !["authorization_code", "refresh_token"].contains(&s.as_str()))
        })
        || r.response_types.is_some_and(|r| r != ["code"])
    {
        return error("invalid_client_metadata", StatusCode::BAD_REQUEST);
    }
    let name = r.client_name.unwrap_or_else(|| "Assistant IA".into());
    if name.len() > 100 || name.chars().any(char::is_control) {
        return error("invalid_client_metadata", StatusCode::BAD_REQUEST);
    }
    let mut s = shared.lock().unwrap();
    s.prune();
    if s.clients.values().filter(|c| c.approved).count() >= 100 {
        return error("registration_limit", StatusCode::TOO_MANY_REQUESTS);
    }
    if s.clients.values().filter(|c| !c.approved).count() >= 32 {
        if let Some(id) = s
            .clients
            .values()
            .filter(|c| !c.approved)
            .min_by_key(|c| c.created)
            .map(|c| c.id.clone())
        {
            s.clients.remove(&id);
        }
    }
    let id = random();
    let secret = (method != "none").then(random);
    s.clients.insert(
        id.clone(),
        Client {
            id: id.clone(),
            name: name.clone(),
            redirects: r.redirect_uris.clone(),
            secret: secret.as_ref().map(|v| hash(v)),
            created: now(),
            approved: false,
        },
    );
    let mut result = json!({"client_id":id,"client_name":name,"redirect_uris":r.redirect_uris,"token_endpoint_auth_method":method,"grant_types":["authorization_code","refresh_token"],"response_types":["code"],"scope":"mcp"});
    if let Some(secret) = secret {
        result["client_secret"] = json!(secret);
        result["client_secret_expires_at"] = json!(0);
    }
    (StatusCode::CREATED, Json(result)).into_response()
}
#[derive(Deserialize)]
struct Authorization {
    client_id: String,
    redirect_uri: String,
    response_type: String,
    code_challenge: String,
    code_challenge_method: String,
    state: Option<String>,
    resource: Option<String>,
    scope: Option<String>,
}
fn page(id: &str, ticket: &str) -> Response {
    let url = format!("/oauth/poll?id={id}&ticket={ticket}");
    let html = format!(
        "<!doctype html><html lang=fr><meta charset=utf-8><meta http-equiv=refresh content=\"2;url={url}\"><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>Connexion — MCP Win</title><style>body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#f5f5fb;color:#1f2937;font:16px system-ui}}main{{max-width:420px;margin:24px;padding:40px;background:white;border:1px solid #e6e5f1;border-radius:20px}}strong{{color:#7050e5}}h1{{font-size:25px;margin-top:30px}}p{{line-height:1.7;color:#6b7280}}small{{color:#7050e5}}</style><main><strong>MCP Win</strong><h1>Autoriser la connexion</h1><p>Ouvrez MCP Win sur votre ordinateur et validez la demande dans Connexion IA.</p><small>En attente de votre accord…</small></main></html>"
    );
    let mut r = axum::response::Html(html).into_response();
    r.headers_mut().insert(
        "content-security-policy",
        "default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"
            .parse()
            .unwrap(),
    );
    r.headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    r
}
async fn authorize(State(shared): State<Shared>, Query(a): Query<Authorization>) -> Response {
    let mut s = shared.lock().unwrap();
    s.prune();
    if a.response_type != "code"
        || a.code_challenge_method != "S256"
        || URL_SAFE_NO_PAD
            .decode(&a.code_challenge)
            .map_or(true, |b| b.len() != 32)
        || a.state.as_ref().is_some_and(|v| v.len() > 2048)
        || a.scope.as_deref().is_some_and(|v| v != "mcp")
        || a.resource.as_ref().is_some_and(|r| r != &s.resource)
    {
        return error("invalid_request", StatusCode::BAD_REQUEST);
    }
    if !s
        .clients
        .get(&a.client_id)
        .is_some_and(|c| c.redirects.contains(&a.redirect_uri))
    {
        return error("invalid_client", StatusCode::BAD_REQUEST);
    }
    if s.requests.len() >= 32 {
        return error("temporarily_unavailable", StatusCode::TOO_MANY_REQUESTS);
    }
    let id = random();
    let ticket = random();
    s.requests.insert(
        id.clone(),
        Request {
            client: a.client_id,
            redirect: a.redirect_uri,
            challenge: a.code_challenge,
            state: a.state.unwrap_or_default(),
            ticket: hash(&ticket),
            expires: now() + 300,
            approved: None,
        },
    );
    page(&id, &ticket)
}
#[derive(Deserialize)]
struct Poll {
    id: String,
    ticket: String,
}
async fn poll(State(shared): State<Shared>, Query(q): Query<Poll>) -> Response {
    let mut s = shared.lock().unwrap();
    s.prune();
    let Some(r) = s.requests.get(&q.id) else {
        return error("expired_request", StatusCode::BAD_REQUEST);
    };
    if !bool::from(r.ticket.as_bytes().ct_eq(hash(&q.ticket).as_bytes())) {
        return error("invalid_request", StatusCode::BAD_REQUEST);
    }
    if r.approved.is_none() {
        return page(&q.id, &q.ticket);
    }
    let r = s.requests.remove(&q.id).unwrap();
    let mut url = reqwest::Url::parse(&r.redirect).unwrap();
    if r.approved == Some(true) {
        let code = random();
        s.codes.insert(
            hash(&code),
            Code {
                client: r.client,
                redirect: r.redirect,
                challenge: r.challenge,
                expires: now() + 120,
            },
        );
        url.query_pairs_mut().append_pair("code", &code);
    } else {
        url.query_pairs_mut().append_pair("error", "access_denied");
    }
    if !r.state.is_empty() {
        url.query_pairs_mut().append_pair("state", &r.state);
    }
    (StatusCode::SEE_OTHER, [("location", url.as_str())]).into_response()
}
fn credentials(
    headers: &HeaderMap,
    id: String,
    secret: Option<String>,
) -> Result<(String, Option<String>), ()> {
    let Some(auth) = headers.get("authorization") else {
        return Ok((id, secret));
    };
    if secret.is_some() {
        return Err(());
    }
    let decoded = STANDARD
        .decode(
            auth.to_str()
                .map_err(|_| ())?
                .strip_prefix("Basic ")
                .ok_or(())?,
        )
        .map_err(|_| ())?;
    let value = std::str::from_utf8(&decoded).map_err(|_| ())?;
    let (user, password) = value.split_once(':').ok_or(())?;
    let decode = |v: &str| -> Result<String, ()> {
        let url = reqwest::Url::parse(&format!("http://localhost/?v={v}")).map_err(|_| ())?;
        let pairs: Vec<_> = url.query_pairs().into_owned().collect();
        if pairs.len() != 1 || pairs[0].0 != "v" {
            return Err(());
        }
        Ok(pairs[0].1.clone())
    };
    let client = decode(user)?;
    if !id.is_empty() && id != client {
        return Err(());
    }
    Ok((client, Some(decode(password)?)))
}
#[derive(Deserialize)]
struct Exchange {
    grant_type: String,
    #[serde(default)]
    client_id: String,
    client_secret: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
    refresh_token: Option<String>,
    resource: Option<String>,
}
async fn exchange(
    State(shared): State<Shared>,
    headers: HeaderMap,
    Form(mut e): Form<Exchange>,
) -> Response {
    let Ok((id, secret)) = credentials(&headers, e.client_id, e.client_secret) else {
        return error("invalid_client", StatusCode::UNAUTHORIZED);
    };
    e.client_id = id;
    e.client_secret = secret;
    let mut s = shared.lock().unwrap();
    s.prune();
    let Some(client) = s.clients.get(&e.client_id) else {
        return error("invalid_client", StatusCode::UNAUTHORIZED);
    };
    if !client.approved
        || client.secret.as_ref().is_some_and(|secret| {
            !bool::from(
                secret
                    .as_bytes()
                    .ct_eq(hash(e.client_secret.as_deref().unwrap_or("")).as_bytes()),
            )
        })
    {
        return error("invalid_client", StatusCode::UNAUTHORIZED);
    }
    if e.resource.as_ref().is_some_and(|r| r != &s.resource) {
        return error("invalid_target", StatusCode::BAD_REQUEST);
    }
    if s.tokens.len() > 1024 || s.used.len() > 2048 {
        return error("temporarily_unavailable", StatusCode::TOO_MANY_REQUESTS);
    }
    match e.grant_type.as_str() {
        "authorization_code" => {
            let Some(code) = s.codes.remove(&hash(e.code.as_deref().unwrap_or(""))) else {
                return error("invalid_grant", StatusCode::BAD_REQUEST);
            };
            let verifier = e.code_verifier.unwrap_or_default();
            if !(43..=128).contains(&verifier.len())
                || !verifier
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-._~".contains(&c))
                || code.client != e.client_id
                || Some(code.redirect) != e.redirect_uri
                || code.challenge != URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
            {
                return error("invalid_grant", StatusCode::BAD_REQUEST);
            }
            Json(s.issue(e.client_id, random())).into_response()
        }
        "refresh_token" => {
            let h = hash(e.refresh_token.as_deref().unwrap_or(""));
            if let Some((client, family, _)) = s.used.get(&h).cloned() {
                if client == e.client_id {
                    s.revoke(&family)
                }
                return error("invalid_grant", StatusCode::BAD_REQUEST);
            }
            let Some(token) = s.tokens.get(&h) else {
                return error("invalid_grant", StatusCode::BAD_REQUEST);
            };
            if !token.refresh || token.client != e.client_id {
                return error("invalid_grant", StatusCode::BAD_REQUEST);
            }
            let token = s.tokens.remove(&h).unwrap();
            s.used.insert(
                h,
                (
                    token.client.clone(),
                    token.family.clone(),
                    now() + 7 * 86400,
                ),
            );
            s.revoke(&token.family);
            Json(s.issue(e.client_id, token.family)).into_response()
        }
        _ => error("unsupported_grant_type", StatusCode::BAD_REQUEST),
    }
}
#[derive(Deserialize)]
struct Revocation {
    token: String,
    #[serde(default)]
    client_id: String,
    client_secret: Option<String>,
}
async fn revoke(
    State(shared): State<Shared>,
    headers: HeaderMap,
    Form(mut r): Form<Revocation>,
) -> Response {
    let Ok((id, secret)) = credentials(&headers, r.client_id, r.client_secret) else {
        return error("invalid_client", StatusCode::UNAUTHORIZED);
    };
    r.client_id = id;
    r.client_secret = secret;
    let mut s = shared.lock().unwrap();
    s.prune();
    if !s.clients.get(&r.client_id).is_some_and(|c| {
        c.secret.as_ref().is_none_or(|h| {
            bool::from(
                h.as_bytes()
                    .ct_eq(hash(r.client_secret.as_deref().unwrap_or("")).as_bytes()),
            )
        })
    }) {
        return error("invalid_client", StatusCode::UNAUTHORIZED);
    }
    if let Some(t) = s.tokens.get(&hash(&r.token)) {
        if t.client == r.client_id {
            let f = t.family.clone();
            s.revoke(&f)
        }
    }
    StatusCode::OK.into_response()
}
async fn metadata(State(shared): State<Shared>) -> Json<Value> {
    let s = shared.lock().unwrap();
    Json(
        json!({"issuer":s.issuer,"authorization_endpoint":format!("{}/oauth/authorize",s.issuer),"token_endpoint":format!("{}/oauth/token",s.issuer),"registration_endpoint":format!("{}/oauth/register",s.issuer),"revocation_endpoint":format!("{}/oauth/revoke",s.issuer),"response_types_supported":["code"],"grant_types_supported":["authorization_code","refresh_token"],"code_challenge_methods_supported":["S256"],"token_endpoint_auth_methods_supported":["none","client_secret_post","client_secret_basic"],"scopes_supported":["mcp"]}),
    )
}
async fn resource(State(shared): State<Shared>) -> Json<Value> {
    let s = shared.lock().unwrap();
    Json(
        json!({"resource":s.resource,"authorization_servers":[s.issuer],"scopes_supported":["mcp"],"bearer_methods_supported":["header"]}),
    )
}
pub fn routes(shared: Shared) -> Router {
    Router::new()
        .route("/.well-known/oauth-authorization-server", get(metadata))
        .route("/.well-known/oauth-protected-resource", get(resource))
        .route("/.well-known/oauth-protected-resource/mcp", get(resource))
        .route("/oauth/register", post(register))
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/poll", get(poll))
        .route("/oauth/token", post(exchange))
        .route("/oauth/revoke", post(revoke))
        .with_state(shared)
}
pub fn is_route(path: &str) -> bool {
    matches!(
        path,
        "/.well-known/oauth-authorization-server"
            | "/.well-known/oauth-protected-resource"
            | "/.well-known/oauth-protected-resource/mcp"
            | "/oauth/register"
            | "/oauth/authorize"
            | "/oauth/poll"
            | "/oauth/token"
            | "/oauth/revoke"
    )
}
pub fn local_snapshot() -> Value {
    let Some(shared) = current().lock().unwrap().clone() else {
        return json!({"requests":[],"clients":[]});
    };
    let mut s = shared.lock().unwrap();
    s.prune();
    json!({"requests":s.requests.iter().filter(|(_,r)|r.approved.is_none()).map(|(id,r)|json!({"id":id,"name":s.clients.get(&r.client).map(|c|&c.name),"redirect_uri":r.redirect,"expires":r.expires})).collect::<Vec<_>>(),"clients":s.clients.values().filter(|c|c.approved).map(|c|json!({"id":c.id,"name":c.name,"created":c.created})).collect::<Vec<_>>()})
}
#[tauri::command]
pub fn oauth_status() -> Value {
    local_snapshot()
}
#[tauri::command]
pub fn oauth_decision(id: String, action: String) -> Result<(), String> {
    let shared = current().lock().unwrap().clone().ok_or("OAuth non actif")?;
    let mut s = shared.lock().unwrap();
    s.prune();
    match action.as_str() {
        "approve" | "deny" => {
            let r = s.requests.get_mut(&id).ok_or("Demande expirée")?;
            if r.approved.is_some() {
                return Err("Demande déjà traitée".into());
            }
            r.approved = Some(action == "approve");
            let client = r.client.clone();
            if action == "approve" {
                s.clients.get_mut(&client).ok_or("Client inconnu")?.approved = true;
                s.persist()?;
            }
        }
        "revoke" => {
            s.clients.remove(&id);
            s.tokens.retain(|_, t| t.client != id);
            s.used.retain(|_, (client, _, _)| client != &id);
            s.codes.retain(|_, c| c.client != id);
            s.requests.retain(|_, r| r.client != id);
            s.persist()?;
        }
        _ => return Err("Action OAuth invalide".into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn oauth_http_consent_pkce_refresh_replay_and_revocation() {
        let dir = tempfile::tempdir().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());
        let cancel = tokio_util::sync::CancellationToken::new();
        let task = tokio::spawn(crate::server::serve_authenticated(
            listener,
            vec![crate::library::SharedFolder {
                id: "oauth-test".into(),
                path: dir.path().into(),
                writable: false,
            }],
            Some("0123456789abcdef0123456789abcdef".into()),
            cancel.clone(),
            Default::default(),
            None,
            true,
        ));
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let mcp = format!("{issuer}/mcp");
        for _ in 0..50 {
            if client
                .get(format!("{issuer}/.well-known/oauth-authorization-server"))
                .send()
                .await
                .is_ok()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
        let missing = client.post(&mcp).body("{}").send().await.unwrap();
        assert_eq!(missing.status(), 401);
        assert!(
            missing.headers()["www-authenticate"]
                .to_str()
                .unwrap()
                .contains("resource_metadata")
        );
        let wrong = client
            .post(&mcp)
            .bearer_auth("0123456789abcdef0123456789abcdef")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(
            wrong.status(),
            401,
            "Static token must not bypass OAuth mode"
        );
        let registration:Value=client.post(format!("{issuer}/oauth/register")).json(&json!({"client_name":"Synthetic assistant","redirect_uris":["https://client.example/callback"],"token_endpoint_auth_method":"none"})).send().await.unwrap().json().await.unwrap();
        let id = registration["client_id"].as_str().unwrap();
        let verifier = "a".repeat(64);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let authorization = client
            .get(format!("{issuer}/oauth/authorize"))
            .query(&[
                ("client_id", id),
                ("redirect_uri", "https://client.example/callback"),
                ("response_type", "code"),
                ("code_challenge", &challenge),
                ("code_challenge_method", "S256"),
                ("resource", &mcp),
                ("state", "synthetic-state"),
            ])
            .send()
            .await
            .unwrap();
        assert_eq!(authorization.status(), 200);
        let snapshot = local_snapshot();
        assert_eq!(snapshot["requests"].as_array().unwrap().len(), 1);
        let html = authorization.text().await.unwrap();
        let link = html
            .split("2;url=")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let waiting = client.get(format!("{issuer}{link}")).send().await.unwrap();
        assert_eq!(waiting.status(), 200, "No redirect before local consent");
        oauth_decision(
            snapshot["requests"][0]["id"].as_str().unwrap().into(),
            "approve".into(),
        )
        .unwrap();
        let redirect = client.get(format!("{issuer}{link}")).send().await.unwrap();
        assert_eq!(redirect.status(), 303);
        let url = reqwest::Url::parse(redirect.headers()["location"].to_str().unwrap()).unwrap();
        let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params["state"], "synthetic-state");
        let exchange = [
            ("grant_type", "authorization_code"),
            ("client_id", id),
            ("code", &params["code"]),
            ("redirect_uri", "https://client.example/callback"),
            ("code_verifier", &verifier),
            ("resource", &mcp),
        ];
        let tokens: Value = client
            .post(format!("{issuer}/oauth/token"))
            .form(&exchange)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let access = tokens["access_token"].as_str().unwrap();
        let refresh = tokens["refresh_token"].as_str().unwrap();
        assert_eq!(
            client
                .post(format!("{issuer}/oauth/token"))
                .form(&exchange)
                .send()
                .await
                .unwrap()
                .status(),
            400,
            "Code is single use"
        );
        let call = client
            .post(&mcp)
            .bearer_auth(access)
            .header("Accept", "application/json, text/event-stream")
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .send()
            .await
            .unwrap();
        assert_eq!(call.status(), 200);
        let renewal = [
            ("grant_type", "refresh_token"),
            ("client_id", id),
            ("refresh_token", refresh),
            ("resource", &mcp),
        ];
        let renewed: Value = client
            .post(format!("{issuer}/oauth/token"))
            .form(&renewal)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(renewed["access_token"].is_string());
        assert_eq!(
            client
                .post(format!("{issuer}/oauth/token"))
                .form(&renewal)
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
        let response = client
            .post(&mcp)
            .bearer_auth(renewed["access_token"].as_str().unwrap())
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401, "Refresh replay revokes the family");
        oauth_decision(id.into(), "revoke".into()).unwrap();
        cancel.cancel();
        task.await.unwrap().unwrap();
        assert!(local_snapshot()["requests"].as_array().unwrap().is_empty());
    }
    #[test]
    fn rejects_unsafe_redirects_and_plain_pkce_input() {
        for uri in [
            "https://client.example/callback#fragment",
            "javascript:alert(1)",
            "http://evil.example/cb",
            "https://user:secret@client.example/cb",
            "https://client.example/cb?code=ambiguous",
        ] {
            assert!(!safe_redirect(uri), "{uri}");
        }
        assert!(safe_redirect("https://client.example/callback"));
        assert!(safe_redirect("http://127.0.0.1:1234/callback"));
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            format!("Basic {}", STANDARD.encode("client:secret"))
                .parse()
                .unwrap(),
        );
        assert_eq!(
            credentials(&headers, String::new(), None).unwrap(),
            ("client".into(), Some("secret".into()))
        );
        assert!(credentials(&headers, "other".into(), None).is_err());
        assert!(credentials(&headers, "client".into(), Some("duplicate".into())).is_err());
    }
}
