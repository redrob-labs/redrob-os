//! Pure policy functions: what a scope allows, how an approval is bound to a request.
use crate::config::Scope;
use sha2::{Digest, Sha256};

/// A request path is allowed when it starts with one of the scope's prefixes and
/// contains no `..` segment (defence against prefix escapes after normalisation).
pub fn path_allowed(scope: &Scope, path: &str) -> bool {
    if !path.starts_with('/') || path.split('/').any(|seg| seg == "..") {
        return false;
    }
    scope.paths.iter().any(|p| path.starts_with(p.as_str()))
}

pub fn method_allowed(scope: &Scope, method: &str) -> bool {
    if scope.methods.is_empty() {
        return method == "GET";
    }
    scope.methods.iter().any(|m| m.eq_ignore_ascii_case(method))
}

/// Egress allow-list match: exact host, or `*.suffix` for any strict subdomain.
pub fn host_allowed(allow: &[String], host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    allow.iter().any(|rule| {
        let rule = rule.to_ascii_lowercase();
        if let Some(suffix) = rule.strip_prefix("*.") {
            host.len() > suffix.len() + 1
                && host.ends_with(suffix)
                && host.as_bytes()[host.len() - suffix.len() - 1] == b'.'
        } else {
            host == rule
        }
    })
}

/// Hash that an approval is bound to: task, scope, method, path, canonical query, body.
pub fn request_hash(
    task: &str,
    scope: &str,
    method: &str,
    path: &str,
    query: &[(String, String)],
    body: &[u8],
) -> String {
    let mut q: Vec<&(String, String)> = query.iter().collect();
    q.sort();
    let mut h = Sha256::new();
    for part in [task, scope, method, path] {
        h.update(part.as_bytes());
        h.update([0u8]);
    }
    for (k, v) in q {
        h.update(k.as_bytes());
        h.update([b'=']);
        h.update(v.as_bytes());
        h.update([0u8]);
    }
    h.update(body);
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RiskClass;

    fn scope() -> Scope {
        Scope {
            base_url: "https://gmail.googleapis.com".into(),
            credential: "google.user".into(),
            auth: "bearer".into(),
            methods: vec![],
            paths: vec!["/gmail/v1/users/me/messages".into()],
            risk: RiskClass::Read,
        }
    }

    #[test]
    fn paths() {
        let s = scope();
        assert!(path_allowed(&s, "/gmail/v1/users/me/messages"));
        assert!(path_allowed(&s, "/gmail/v1/users/me/messages/123"));
        assert!(!path_allowed(&s, "/gmail/v1/users/me/drafts"));
        assert!(!path_allowed(&s, "/gmail/v1/users/me/messages/../drafts"));
        assert!(!path_allowed(&s, "gmail/v1/users/me/messages"));
    }

    #[test]
    fn methods_default_get_only() {
        let mut s = scope();
        assert!(method_allowed(&s, "GET"));
        assert!(!method_allowed(&s, "POST"));
        s.methods = vec!["post".into()];
        assert!(method_allowed(&s, "POST"));
        assert!(!method_allowed(&s, "GET"));
    }

    #[test]
    fn hosts() {
        let allow = vec!["*.googleapis.com".into(), "github.com".into()];
        assert!(host_allowed(&allow, "gmail.googleapis.com"));
        assert!(host_allowed(&allow, "GitHub.com"));
        assert!(!host_allowed(&allow, "googleapis.com"));
        assert!(!host_allowed(&allow, "evilgithub.com"));
        assert!(!host_allowed(&allow, "github.com.evil.net"));
    }

    #[test]
    fn hash_is_order_independent_for_query_and_body_sensitive() {
        let a = request_hash(
            "t",
            "s",
            "POST",
            "/p",
            &[("b".into(), "2".into()), ("a".into(), "1".into())],
            b"x",
        );
        let b = request_hash(
            "t",
            "s",
            "POST",
            "/p",
            &[("a".into(), "1".into()), ("b".into(), "2".into())],
            b"x",
        );
        let c = request_hash(
            "t",
            "s",
            "POST",
            "/p",
            &[("a".into(), "1".into()), ("b".into(), "2".into())],
            b"y",
        );
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
