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
        Ok(n) => Json(json!({"revoked": n})).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
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

    let credential = match s.store.lock().await.credential(&scope.credential) {
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
        Err(e) => {
            tracing::error!(%e, "credential decrypt");
            return refuse(
                &s,
                &b,
                &method,
                &host,
                peer,
                "credential-unreadable",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
            .await;
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
    let mut req = s.http.request(req_method, &url).query(&query);
    req = if scope.auth == "bearer" {
        req.bearer_auth(&credential)
    } else {
        req.header(
            scope.auth.trim_start_matches("header:"),
            credential.as_str(),
        )
    };
    if !body_bytes.is_empty() {
        req = req
            .header("content-type", "application/json")
            .body(body_bytes.clone());
    }

    let resp = match req.send().await {
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
