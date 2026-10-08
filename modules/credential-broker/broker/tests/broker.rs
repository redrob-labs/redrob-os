//! End-to-end: broker on a temp Unix socket, a mock vendor on loopback, raw HTTP/1.1 client.
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use redrob_broker::api::{self, AppState};
use redrob_broker::audit::Audit;
use redrob_broker::config::{BrokerConfig, EgressConfig, RiskClass, Scope};
use redrob_broker::store::Store;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UnixListener, UnixStream};

/// Mock vendor: echoes method, path, query and the Authorization header as JSON.
async fn mock_vendor() -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let h = tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let svc = service_fn(|req: hyper::Request<hyper::body::Incoming>| async move {
                    let auth = req
                        .headers()
                        .get("authorization")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    let x = req
                        .headers()
                        .get("x-api-key")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    let (method, uri) = (req.method().to_string(), req.uri().to_string());
                    let body = req.into_body().collect().await.unwrap().to_bytes();
                    let out = json!({"method": method, "uri": uri, "authorization": auth, "x_api_key": x, "body": String::from_utf8_lossy(&body)});
                    Ok::<_, hyper::Error>(
                        hyper::Response::builder()
                            .header("content-type", "application/json")
                            .body(Full::new(Bytes::from(out.to_string())))
                            .unwrap(),
                    )
                });
                let _ = http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), svc)
                    .await;
            });
        }
    });
    (format!("http://{addr}"), h)
}

fn scope(base: &str, risk: RiskClass, methods: &[&str], cred: &str, auth: &str) -> Scope {
    Scope {
        base_url: base.into(),
        credential: cred.into(),
        auth: auth.into(),
        methods: methods.iter().map(|m| m.to_string()).collect(),
        paths: vec!["/gmail/v1/users/me/messages".into()],
        risk,
    }
}

struct Harness {
    sock: std::path::PathBuf,
    audit_dir: std::path::PathBuf,
    _dir: tempfile::TempDir,
    _task: tokio::task::JoinHandle<()>,
}

async fn start(vendor: &str, admin_uid: u32) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("broker.sock");
    let audit_dir = dir.path().join("audit");
    let mut scopes = BTreeMap::new();
    scopes.insert(
        "gmail.read".to_string(),
        scope(vendor, RiskClass::Read, &[], "google.user", "bearer"),
    );
    scopes.insert(
        "gmail.send".to_string(),
        scope(
            vendor,
            RiskClass::ExternalSend,
            &["POST"],
            "google.user",
            "bearer",
        ),
    );
    scopes.insert(
        "vendor.key".to_string(),
        scope(
            vendor,
            RiskClass::Mutate,
            &["GET", "PUT"],
            "vendor.apikey",
            "header:X-Api-Key",
        ),
    );
    let cfg = BrokerConfig {
        socket: sock.clone(),
        state_dir: dir.path().join("state"),
        audit_dir: audit_dir.clone(),
        admin_user: "nobody".into(),
        timeout_secs: 5,
        egress: None,
        scopes,
    };
    let store = Store::open(&cfg.state_dir).unwrap();
    let audit = Arc::new(Audit::new(audit_dir.clone()).unwrap());
    let state = Arc::new(AppState {
        cfg,
        store: tokio::sync::Mutex::new(store),
        audit,
        http: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
        admin_uid,
    });
    let listener = UnixListener::bind(&sock).unwrap();
    let task = tokio::spawn(async move {
        api::serve(state, listener).await.unwrap();
    });
    Harness {
        sock,
        audit_dir,
        _dir: dir,
        _task: task,
    }
}

async fn req(h: &Harness, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    let mut s = UnixStream::connect(&h.sock).await.unwrap();
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: broker\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).await.unwrap();
    s.write_all(body.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf).into_owned();
    let status: u16 = text.split_whitespace().nth(1).unwrap().parse().unwrap();
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("");
    let body = body.rsplit("\r\n").find(|p| !p.is_empty()).unwrap_or(body); // chunked bodies: last chunk payload
    let val = serde_json::from_str(body).unwrap_or(Value::Null);
    (status, val)
}

fn my_uid() -> u32 {
    std::fs::metadata("/proc/self")
        .map(|m| std::os::unix::fs::MetadataExt::uid(&m))
        .unwrap()
}

fn audit_lines(h: &Harness) -> Vec<Value> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(&h.audit_dir).unwrap() {
        let p = e.unwrap().path();
        for l in std::fs::read_to_string(p).unwrap().lines() {
            out.push(serde_json::from_str(l).unwrap());
        }
    }
    out
}

#[tokio::test]
async fn scoped_call_injects_credential_and_enforces_grant_path_method() {
    let (vendor, _v) = mock_vendor().await;
    let h = start(&vendor, my_uid()).await;

    let (st, _) = req(
        &h,
        "PUT",
        "/v1/credentials/google.user",
        Some(json!({"value": "ya29.secret"})),
    )
    .await;
    assert_eq!(st, 204);

    // no grant yet
    let (st, b) = req(
        &h,
        "POST",
        "/v1/call",
        Some(json!({"task": "t1", "scope": "gmail.read", "path": "/gmail/v1/users/me/messages"})),
    )
    .await;
    assert_eq!((st, b["error"].as_str()), (403, Some("no-grant")));

    let (st, _) = req(
        &h,
        "POST",
        "/v1/grants",
        Some(json!({"task": "t1", "scopes": ["gmail.read", "gmail.send"]})),
    )
    .await;
    assert_eq!(st, 201);

    let (st, b) = req(&h, "POST", "/v1/call", Some(json!({"task": "t1", "scope": "gmail.read", "path": "/gmail/v1/users/me/messages", "query": {"q": "is:unread"}}))).await;
    assert_eq!(st, 200, "{b}");
    assert_eq!(b["status"], 200);
    assert_eq!(b["body"]["authorization"], "Bearer ya29.secret");
    assert_eq!(
        b["body"]["uri"],
        "/gmail/v1/users/me/messages?q=is%3Aunread"
    );
    // the raw token never appears in the reply except as echoed by the mock vendor body
    assert!(
        b["content_type"]
            .as_str()
            .unwrap()
            .starts_with("application/json")
    );

    // path outside the scope
    let (st, b) = req(
        &h,
        "POST",
        "/v1/call",
        Some(json!({"task": "t1", "scope": "gmail.read", "path": "/gmail/v1/users/me/drafts"})),
    )
    .await;
    assert_eq!((st, b["error"].as_str()), (403, Some("path-not-allowed")));
    // method outside the scope (read scope is GET only)
    let (st, b) = req(&h, "POST", "/v1/call", Some(json!({"task": "t1", "scope": "gmail.read", "method": "DELETE", "path": "/gmail/v1/users/me/messages/1"}))).await;
    assert_eq!((st, b["error"].as_str()), (403, Some("method-not-allowed")));
    // another task has no grant
    let (st, b) = req(
        &h,
        "POST",
        "/v1/call",
        Some(json!({"task": "t2", "scope": "gmail.read", "path": "/gmail/v1/users/me/messages"})),
    )
    .await;
    assert_eq!((st, b["error"].as_str()), (403, Some("no-grant")));

    let lines = audit_lines(&h);
    assert!(
        lines
            .iter()
            .any(|l| l["decision"] == "auto" && l["status"] == 200 && l["scope"] == "gmail.read")
    );
    assert!(lines.iter().any(|l| l["decision"] == "refused:no-grant"));
    assert!(
        lines
            .iter()
            .any(|l| l["decision"] == "refused:path-not-allowed")
    );
    let raw = lines.iter().map(|l| l.to_string()).collect::<String>();
    assert!(
        !raw.contains("ya29.secret"),
        "audit log must never contain a credential"
    );
}

#[tokio::test]
async fn external_send_requires_single_use_approval_bound_to_request() {
    let (vendor, _v) = mock_vendor().await;
    let h = start(&vendor, my_uid()).await;
    req(
        &h,
        "PUT",
        "/v1/credentials/google.user",
        Some(json!({"value": "tok"})),
    )
    .await;
    req(
        &h,
        "POST",
        "/v1/grants",
        Some(json!({"task": "t1", "scopes": ["gmail.send"]})),
    )
    .await;

    let call = json!({"task": "t1", "scope": "gmail.send", "method": "POST", "path": "/gmail/v1/users/me/messages/send", "body": {"raw": "abc"}});
    let (st, b) = req(&h, "POST", "/v1/call", Some(call.clone())).await;
    assert_eq!((st, b["error"].as_str()), (403, Some("approval-required")));
    let hash = b["request_hash"].as_str().unwrap().to_string();
    assert_eq!(hash.len(), 64);

    // approving a different hash does not unlock this request
    req(
        &h,
        "POST",
        "/v1/approvals",
        Some(json!({"task": "t1", "request_hash": "00"})),
    )
    .await;
    let (st, _) = req(&h, "POST", "/v1/call", Some(call.clone())).await;
    assert_eq!(st, 403);

    req(
        &h,
        "POST",
        "/v1/approvals",
        Some(json!({"task": "t1", "request_hash": hash})),
    )
    .await;
    let (st, b) = req(&h, "POST", "/v1/call", Some(call.clone())).await;
    assert_eq!(st, 200, "{b}");
    assert_eq!(b["body"]["method"], "POST");
    assert_eq!(b["body"]["body"], "{\"raw\":\"abc\"}");
    // single use
    let (st, b) = req(&h, "POST", "/v1/call", Some(call)).await;
    assert_eq!((st, b["error"].as_str()), (403, Some("approval-required")));
    assert!(audit_lines(&h).iter().any(|l| l["decision"] == "approved"));
}

#[tokio::test]
async fn header_auth_and_missing_credential() {
    let (vendor, _v) = mock_vendor().await;
    let h = start(&vendor, my_uid()).await;
    req(
        &h,
        "POST",
        "/v1/grants",
        Some(json!({"task": "t1", "scopes": ["vendor.key"]})),
    )
    .await;
    let (st, b) = req(
        &h,
        "POST",
        "/v1/call",
        Some(json!({"task": "t1", "scope": "vendor.key", "path": "/gmail/v1/users/me/messages"})),
    )
    .await;
    assert_eq!((st, b["error"].as_str()), (409, Some("credential-missing")));
    req(
        &h,
        "PUT",
        "/v1/credentials/vendor.apikey",
        Some(json!({"value": "k-1"})),
    )
    .await;
    let (st, b) = req(&h, "POST", "/v1/call", Some(json!({"task": "t1", "scope": "vendor.key", "method": "PUT", "path": "/gmail/v1/users/me/messages/x"}))).await;
    assert_eq!(st, 200, "{b}");
    assert_eq!(b["body"]["x_api_key"], "k-1");
    assert_eq!(b["body"]["authorization"], "");
}

#[tokio::test]
async fn non_admin_peer_cannot_manage_but_can_call() {
    let (vendor, _v) = mock_vendor().await;
    // admin uid is someone else: our test process must be refused on admin routes
    let h = start(&vendor, my_uid().wrapping_add(12345)).await;
    for (m, p, body) in [
        ("PUT", "/v1/credentials/x", Some(json!({"value": "v"}))),
        ("GET", "/v1/credentials", None),
        (
            "POST",
            "/v1/grants",
            Some(json!({"task": "t", "scopes": ["gmail.read"]})),
        ),
        (
            "POST",
            "/v1/approvals",
            Some(json!({"task": "t", "request_hash": "h"})),
        ),
        ("DELETE", "/v1/grants/t", None),
    ] {
        let (st, _) = req(&h, m, p, body).await;
        assert_eq!(st, 403, "{m} {p}");
    }
    let (st, b) = req(&h, "GET", "/v1/health", None).await;
    assert_eq!((st, b["ok"].as_bool()), (200, Some(true)));
    let (st, b) = req(
        &h,
        "POST",
        "/v1/call",
        Some(json!({"task": "t", "scope": "gmail.read", "path": "/gmail/v1/users/me/messages"})),
    )
    .await;
    assert_eq!((st, b["error"].as_str()), (403, Some("no-grant")));
}

#[tokio::test]
async fn egress_proxy_allows_listed_hosts_only_and_audits() {
    let (origin, _o) = mock_vendor().await;
    let origin_host = origin.trim_start_matches("http://").to_string();
    let dir = tempfile::tempdir().unwrap();
    let audit = Arc::new(Audit::new(dir.path().join("audit")).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    drop(listener);
    let cfg = EgressConfig {
        listen: proxy_addr.to_string(),
        allow: vec!["127.0.0.1".into(), "*.googleapis.com".into()],
    };
    let a2 = audit.clone();
    tokio::spawn(async move { redrob_broker::proxy::serve(cfg, a2).await.unwrap() });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("http://{proxy_addr}")).unwrap())
        .build()
        .unwrap();
    let r = client
        .get(format!("http://{origin_host}/ok?x=1"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["uri"], "/ok?x=1");

    // a host that is not on the list: refused by the proxy with 403, never contacted
    let r = client
        .get("http://example.invalid/secret")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
    // CONNECT to a disallowed host is refused as well
    let r = client.get("https://example.invalid/").send().await;
    assert!(r.is_err() || r.unwrap().status() == 403);

    let mut lines = Vec::new();
    for e in std::fs::read_dir(dir.path().join("audit")).unwrap() {
        for l in std::fs::read_to_string(e.unwrap().path()).unwrap().lines() {
            lines.push(serde_json::from_str::<Value>(l).unwrap());
        }
    }
    assert!(
        lines
            .iter()
            .any(|l| l["scope"] == "egress" && l["host"] == "127.0.0.1" && l["status"] == 200)
    );
    assert!(
        lines
            .iter()
            .any(|l| l["host"] == "example.invalid" && l["decision"] == "refused:host-not-allowed")
    );
}

#[test]
fn shipped_config_parses_and_validates() {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../deploy/config/broker.toml"
    );
    let cfg = BrokerConfig::load(std::path::Path::new(p)).expect("deploy/config/broker.toml");
    assert!(cfg.scopes.len() >= 10);
    assert!(
        cfg.egress
            .as_ref()
            .unwrap()
            .allow
            .iter()
            .any(|h| h == "console.redrob.ai")
    );
    // every scope's credential name is one of the documented ones
    for (name, s) in &cfg.scopes {
        assert!(
            [
                "google.user",
                "slack.bot",
                "discord.bot",
                "github.pat",
                "console.apikey"
            ]
            .contains(&s.credential.as_str()),
            "{name}"
        );
    }
}
