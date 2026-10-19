//! HTTP/1.1 API on the Unix socket. Peer identity comes from SO_PEERCRED.
use crate::audit::{Audit, Entry, now_iso};
use crate::config::BrokerConfig;
use crate::policy;
use crate::store::Store;
use anyhow::Result;
use axum::extract::connect_info::{ConnectInfo, Connected};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::serve::IncomingStream;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio::sync::Mutex;

#[derive(Clone, Copy, Debug)]
pub struct Peer {
    pub uid: u32,
}

impl Connected<IncomingStream<'_, UnixListener>> for Peer {
    fn connect_info(stream: IncomingStream<'_, UnixListener>) -> Self {
        match stream.io().peer_cred() {
            Ok(c) => Peer { uid: c.uid() },
            Err(_) => Peer { uid: u32::MAX },
        }
    }
}

pub struct AppState {
    pub cfg: BrokerConfig,
    pub store: Mutex<Store>,
    pub audit: Arc<Audit>,
    pub http: reqwest::Client,
    pub admin_uid: u32,
    /// Per-task egress proxies; `None` when `[egress]` is not configured.
    pub task_egress: Option<Arc<crate::proxy::TaskProxies>>,
}

type Shared = Arc<AppState>;

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/credentials", get(list_credentials))
        .route(
            "/v1/credentials/{name}",
            put(put_credential).delete(delete_credential),
        )
        .route("/v1/grants", get(list_grants).post(post_grant))
        .route("/v1/grants/{task}", axum::routing::delete(revoke_task))
        .route("/v1/approvals", post(post_approval))
        .route("/v1/egress", get(list_egress).post(post_egress))
        .route("/v1/egress/{task}", axum::routing::delete(delete_egress))
        .route("/v1/call", post(call))
        .with_state(state)
}

pub async fn serve(state: Shared, listener: UnixListener) -> Result<()> {
    let app = router(state).into_make_service_with_connect_info::<Peer>();
    axum::serve(listener, app).await?;
    Ok(())
}

fn err(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(json!({"error": msg.into()}))).into_response()
}

fn require_admin(state: &AppState, peer: &Peer) -> Result<(), Response> {
    if peer.uid == state.admin_uid || peer.uid == 0 {
        Ok(())
    } else {
        Err(err(
            StatusCode::FORBIDDEN,
            format!("uid {} is not the broker admin", peer.uid),
        ))
    }
}

async fn health(State(s): State<Shared>) -> Json<Value> {
    let store = s.store.lock().await;
    Json(json!({
        "ok": true,
        "scopes": s.cfg.scopes.len(),
        "credentials": store.credential_names().len(),
        "grants": store.grants().len(),
    }))
}

#[derive(Deserialize)]
struct CredentialBody {
    value: String,
}

async fn put_credential(
    State(s): State<Shared>,
    ConnectInfo(peer): ConnectInfo<Peer>,
    Path(name): Path<String>,
    Json(b): Json<CredentialBody>,
) -> Response {
    if let Err(r) = require_admin(&s, &peer) {
        return r;
    }
    if name.is_empty() || b.value.is_empty() {
        return err(StatusCode::BAD_REQUEST, "name and value required");
    }
    match s.store.lock().await.set_credential(&name, &b.value) {
        Ok(()) => (StatusCode::NO_CONTENT, ()).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn delete_credential(
    State(s): State<Shared>,
    ConnectInfo(peer): ConnectInfo<Peer>,
    Path(name): Path<String>,
) -> Response {
    if let Err(r) = require_admin(&s, &peer) {
        return r;
    }
    match s.store.lock().await.delete_credential(&name) {
        Ok(true) => (StatusCode::NO_CONTENT, ()).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "no such credential"),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn list_credentials(
    State(s): State<Shared>,
    ConnectInfo(peer): ConnectInfo<Peer>,
) -> Response {
    if let Err(r) = require_admin(&s, &peer) {
        return r;
    }
    // names only -- values never leave the broker
    Json(json!({"credentials": s.store.lock().await.credential_names()})).into_response()
}

#[derive(Deserialize)]
struct GrantBody {
    task: String,
    scopes: Vec<String>,
    #[serde(default = "default_ttl")]
    ttl_secs: u64,
}
fn default_ttl() -> u64 {
    3600
}

async fn post_grant(
    State(s): State<Shared>,
    ConnectInfo(peer): ConnectInfo<Peer>,
    Json(b): Json<GrantBody>,
) -> Response {
    if let Err(r) = require_admin(&s, &peer) {
        return r;
    }
    if let Some(unknown) = b.scopes.iter().find(|sc| !s.cfg.scopes.contains_key(*sc)) {
        return err(StatusCode::BAD_REQUEST, format!("unknown scope {unknown}"));
    }
    match s.store.lock().await.grant(
        &b.task,
        &b.scopes,
        b.ttl_secs.min(86_400),
        &format!("uid:{}", peer.uid),
    ) {
        Ok(g) => (StatusCode::CREATED, Json(json!({"grants": g}))).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn list_grants(State(s): State<Shared>, ConnectInfo(peer): ConnectInfo<Peer>) -> Response {
    if let Err(r) = require_admin(&s, &peer) {
        return r;
    }
    Json(json!({"grants": s.store.lock().await.grants()})).into_response()
}

async fn revoke_task(
    State(s): State<Shared>,
    ConnectInfo(peer): ConnectInfo<Peer>,
    Path(task): Path<String>,
) -> Response {
    if let Err(r) = require_admin(&s, &peer) {
        return r;
    }
    match s.store.lock().await.revoke_task(&task) {
        Ok(n) => {
            // Revoking a task also closes its network: no grants, no egress.
            let egress = s.task_egress.as_ref().is_some_and(|t| t.stop(&task));
            Json(json!({"revoked": n, "egress_closed": egress})).into_response()
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[derive(Deserialize)]
struct EgressBody {
    task: String,
    allow: Vec<String>,
    /// Seconds until the proxy closes; capped at a day. Default one hour.
    #[serde(default = "default_ttl")]
    ttl_secs: u64,
}

fn task_egress(
    s: &AppState,
) -> Result<(&crate::config::EgressConfig, &crate::proxy::TaskProxies), Response> {
    match (&s.cfg.egress, &s.task_egress) {
        (Some(cfg), Some(t)) => Ok((cfg, t)),
        _ => Err(err(StatusCode::NOT_FOUND, "egress is not configured")),
    }
}

async fn post_egress(
    State(s): State<Shared>,
    ConnectInfo(peer): ConnectInfo<Peer>,
    Json(b): Json<EgressBody>,
) -> Response {
    if let Err(r) = require_admin(&s, &peer) {
        return r;
    }
    let (cfg, tasks) = match task_egress(&s) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if !crate::config::valid_task_name(&b.task) {
        return err(StatusCode::BAD_REQUEST, "bad task name");
    }
    if let Some(rule) = b
        .allow
        .iter()
        .find(|r| !crate::config::task_rule_admitted(&cfg.allow, r))
    {
        return err(
            StatusCode::FORBIDDEN,
            format!("{rule} is not admitted by [egress] allow"),
        );
    }
    let ttl = b.ttl_secs.clamp(1, 86_400);
    match tasks.start(&b.task, b.allow, ttl) {
        Ok(path) => (
            StatusCode::CREATED,
            Json(json!({"task": b.task, "socket": path, "ttl_secs": ttl})),
        )
            .into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn list_egress(State(s): State<Shared>, ConnectInfo(peer): ConnectInfo<Peer>) -> Response {
    if let Err(r) = require_admin(&s, &peer) {
        return r;
    }
    let tasks = s
        .task_egress
        .as_ref()
        .map(|t| t.tasks())
        .unwrap_or_default();
    Json(json!({"tasks": tasks})).into_response()
}

async fn delete_egress(
    State(s): State<Shared>,
    ConnectInfo(peer): ConnectInfo<Peer>,
    Path(task): Path<String>,
) -> Response {
    if let Err(r) = require_admin(&s, &peer) {
        return r;
    }
    match s.task_egress.as_ref().map(|t| t.stop(&task)) {
        Some(true) => (StatusCode::NO_CONTENT, ()).into_response(),
        _ => err(StatusCode::NOT_FOUND, "no egress for that task"),
    }
}

#[derive(Deserialize)]
struct ApprovalBody {
    task: String,
    request_hash: String,
}

async fn post_approval(
    State(s): State<Shared>,
    ConnectInfo(peer): ConnectInfo<Peer>,
    Json(b): Json<ApprovalBody>,
) -> Response {
    if let Err(r) = require_admin(&s, &peer) {
        return r;
    }
    s.store.lock().await.approve(&b.task, &b.request_hash);
    (StatusCode::NO_CONTENT, ()).into_response()
}

#[derive(Deserialize)]
pub struct CallBody {
    pub task: String,
    pub scope: String,
    #[serde(default = "default_method")]
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub query: BTreeMap<String, String>,
    #[serde(default)]
    pub body: Option<Value>,
}
fn default_method() -> String {
    "GET".into()
}

#[derive(Serialize)]
struct CallReply {
    status: u16,
    content_type: Option<String>,
    body: Value,
}

async fn call(
    State(s): State<Shared>,
    ConnectInfo(peer): ConnectInfo<Peer>,
    Json(b): Json<CallBody>,
) -> Response {
    let method = b.method.to_ascii_uppercase();
    let body_bytes = match &b.body {
        Some(v) => serde_json::to_vec(v).unwrap_or_default(),
        None => Vec::new(),
    };
    let query: Vec<(String, String)> = b
        .query
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let hash = policy::request_hash(&b.task, &b.scope, &method, &b.path, &query, &body_bytes);

    let Some(scope) = s.cfg.scopes.get(&b.scope) else {
        return refuse(
            &s,
            &b,
            &method,
            "",
            peer,
            "unknown-scope",
            StatusCode::NOT_FOUND,
        )
        .await;
    };
    let host = reqwest::Url::parse(&scope.base_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_default();

    if !s.store.lock().await.has_grant(&b.task, &b.scope) {
        return refuse(
            &s,
            &b,
            &method,
            &host,
            peer,
            "no-grant",
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    if !policy::method_allowed(scope, &method) {
        return refuse(
            &s,
            &b,
            &method,
            &host,
            peer,
            "method-not-allowed",
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    if !policy::path_allowed(scope, &b.path) {
        return refuse(
            &s,
            &b,
            &method,
            &host,
            peer,
            "path-not-allowed",
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    let decision = if scope.risk.needs_approval() {
        if s.store.lock().await.take_approval(&b.task, &hash) {
            "approved"
        } else {
            audit_line(
                &s,
                &b,
                &method,
                &host,
                peer,
                0,
                0,
                0,
                "refused:approval-required",
            );
            return (
                StatusCode::FORBIDDEN,
                Json(
                    json!({"error": "approval-required", "request_hash": hash, "risk": scope.risk}),
                ),
            )
                .into_response();
        }
    } else {
        "auto"
    };

    let (credential, is_oauth) = match resolve_credential(&s, &scope.credential, false).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return refuse(
                &s,
                &b,
                &method,
                &host,
                peer,
                "credential-missing",
                StatusCode::CONFLICT,
            )
            .await;
        }
        Err((status, reason)) => {
            return refuse(&s, &b, &method, &host, peer, reason, status).await;
        }
    };

    let url = format!("{}{}", scope.base_url.trim_end_matches('/'), b.path);
    let req_method = match reqwest::Method::from_bytes(method.as_bytes()) {
        Ok(m) => m,
        Err(_) => {
            return refuse(
                &s,
                &b,
                &method,
                &host,
                peer,
                "bad-method",
                StatusCode::BAD_REQUEST,
            )
            .await;
        }
    };
    let build = |token: &str| {
        let mut req = s.http.request(req_method.clone(), &url).query(&query);
        req = if scope.auth == "bearer" {
            req.bearer_auth(token)
        } else {
            req.header(scope.auth.trim_start_matches("header:"), token)
        };
        if !body_bytes.is_empty() {
            req = req
                .header("content-type", "application/json")
                .body(body_bytes.clone());
        }
        req
    };

    let mut sent = build(&credential).send().await;
    // An OAuth access token can be revoked or expire early: refresh once and retry.
    if is_oauth && matches!(&sent, Ok(r) if r.status() == reqwest::StatusCode::UNAUTHORIZED) {
        match resolve_credential(&s, &scope.credential, true).await {
            Ok(Some((fresh, _))) => sent = build(&fresh).send().await,
            Ok(None) => {}
            Err((status, reason)) => {
                return refuse(&s, &b, &method, &host, peer, reason, status).await;
            }
        }
    }

    let resp = match sent {
        Ok(r) => r,
        Err(e) => {
            audit_line(
                &s,
                &b,
                &method,
                &host,
                peer,
                502,
                body_bytes.len(),
                0,
                decision,
            );
            return err(StatusCode::BAD_GATEWAY, format!("upstream: {e}"));
        }
    };
    let status = resp.status().as_u16();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = resp.bytes().await.unwrap_or_default();
    audit_line(
        &s,
        &b,
        &method,
        &host,
        peer,
        status,
        body_bytes.len(),
        bytes.len(),
        decision,
    );
    let body = serde_json::from_slice::<Value>(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    Json(CallReply {
        status,
        content_type,
        body,
    })
    .into_response()
}

/// The token to send for `name`, and whether it is a refreshable OAuth credential.
/// OAuth credentials are refreshed when close to expiry, or always with `force`;
/// the store lock is held across the refresh so concurrent calls refresh once.
async fn resolve_credential(
    s: &AppState,
    name: &str,
    force: bool,
) -> Result<Option<(String, bool)>, (StatusCode, &'static str)> {
    let mut store = s.store.lock().await;
    let raw = match store.credential(name) {
        Ok(Some(v)) => v,
        Ok(None) => return Ok(None),
        Err(e) => {
            tracing::error!(%e, "credential decrypt");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "credential-unreadable"));
        }
    };
    let Some(mut oauth) = crate::oauth::OAuthCredential::parse(&raw) else {
        return Ok(Some((raw, false)));
    };
    let now = crate::store::now_secs();
    if force || oauth.needs_refresh(now) {
        if let Err(e) = oauth.refresh(&s.http, now).await {
            tracing::warn!(credential = name, %e, "oauth refresh failed");
            return Err((StatusCode::BAD_GATEWAY, "credential-refresh-failed"));
        }
        let saved = oauth.to_json().and_then(|v| store.set_credential(name, &v));
        if let Err(e) = saved {
            // The fresh token still works for this call; the next one refreshes again.
            tracing::error!(credential = name, %e, "oauth token not saved");
        }
    }
    Ok(Some((oauth.access_token, true)))
}

async fn refuse(
    s: &AppState,
    b: &CallBody,
    method: &str,
    host: &str,
    peer: Peer,
    reason: &str,
    status: StatusCode,
) -> Response {
    audit_line(
        s,
        b,
        method,
        host,
        peer,
        0,
        0,
        0,
        &format!("refused:{reason}"),
    );
    err(status, reason)
}

#[allow(clippy::too_many_arguments)]
fn audit_line(
    s: &AppState,
    b: &CallBody,
    method: &str,
    host: &str,
    peer: Peer,
    status: u16,
    bytes_in: usize,
    bytes_out: usize,
    decision: &str,
) {
    s.audit.write(&Entry {
        ts: now_iso(),
        task: &b.task,
        scope: &b.scope,
        method,
        host,
        path: &b.path,
        status,
        bytes_in,
        bytes_out,
        decision,
        peer_uid: peer.uid,
    });
}
