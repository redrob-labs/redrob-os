//! TLS and mutual TLS (mTLS) support for the gateway server.
//!
//! Thin adapter over the shared [`zeroclaw_tls`] crate: it maps the gateway's
//! [`GatewayTlsConfig`] onto the transport-neutral
//! [`zeroclaw_tls::ServerConfigParams`] and delegates construction. The reusable
//! verifier / certificate-pinning / PEM-loading logic lives in `zeroclaw-tls` so
//! it can also be consumed by `zeroclaw-runtime` without an upward dependency on
//! the gateway.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio_rustls::TlsAcceptor;
use zeroclaw_config::schema::GatewayTlsConfig;
use zeroclaw_tls::{ClientAuthParams, ServerConfigParams};

/// Directory under the agent data dir that holds the self-provisioned gateway
/// certificate (`[gateway.tls] enabled = true` with empty `cert_path`/`key_path`).
pub const AUTO_TLS_DIR: &str = "gateway-tls";

/// True when the config asks the gateway to self-provision its certificate.
fn is_auto(config: &GatewayTlsConfig) -> bool {
    config.cert_path.is_empty() && config.key_path.is_empty()
}

/// The certificate file this config serves, without generating anything.
fn served_cert_path(config: &GatewayTlsConfig, data_dir: &Path) -> PathBuf {
    if is_auto(config) {
        data_dir.join(AUTO_TLS_DIR).join("server.crt")
    } else {
        PathBuf::from(&config.cert_path)
    }
}

/// Resolve the cert/key pair to serve: the configured pair, or — when both
/// paths are empty — the self-provisioned pair under `<data_dir>/gateway-tls/`,
/// generated on first call and reused on every later start.
fn resolve_cert_key(config: &GatewayTlsConfig, data_dir: &Path) -> Result<(String, String)> {
    if is_auto(config) {
        let mats = zeroclaw_tls::ensure_server_materials(&data_dir.join(AUTO_TLS_DIR), &[])
            .context("self-provisioning the gateway TLS certificate")?;
        return Ok((
            mats.server_cert_path.to_string_lossy().into_owned(),
            mats.server_key_path.to_string_lossy().into_owned(),
        ));
    }
    if config.cert_path.is_empty() || config.key_path.is_empty() {
        anyhow::bail!(
            "[gateway.tls] set both cert_path and key_path, or leave both empty to self-provision"
        );
    }
    Ok((config.cert_path.clone(), config.key_path.clone()))
}

/// Build a [`TlsAcceptor`] from the gateway TLS configuration.
pub fn build_tls_acceptor(config: &GatewayTlsConfig, data_dir: &Path) -> Result<TlsAcceptor> {
    zeroclaw_tls::build_tls_acceptor(&to_params(config, data_dir)?)
}

/// Build a [`rustls::ServerConfig`] from the gateway TLS configuration.
pub fn build_server_config(
    config: &GatewayTlsConfig,
    data_dir: &Path,
) -> Result<rustls::ServerConfig> {
    zeroclaw_tls::build_server_config(&to_params(config, data_dir)?)
}

/// Hex SHA-256 fingerprint of the leaf certificate the gateway serves, so a
/// pairing client can pin it. Read from the certificate file at call time (the
/// file is the source of truth); `None` when it does not exist yet or is
/// unreadable.
pub fn served_cert_fingerprint(config: &GatewayTlsConfig, data_dir: &Path) -> Option<String> {
    let path = served_cert_path(config, data_dir);
    let certs = zeroclaw_tls::load_certs(path.to_str()?).ok()?;
    certs
        .first()
        .map(|c| zeroclaw_tls::cert_sha256_fingerprint(c.as_ref()))
}

/// Map the gateway TLS config onto the transport-neutral params.
///
/// Client authentication is enabled only when the `[gateway.tls.client_auth]`
/// block is present AND `enabled = true`, preserving the previous
/// `.filter(|ca| ca.enabled)` behavior (a `client_auth` with `enabled = false`
/// is treated as server-only TLS, skipping CA loading).
fn to_params(config: &GatewayTlsConfig, data_dir: &Path) -> Result<ServerConfigParams> {
    let (cert_path, key_path) = resolve_cert_key(config, data_dir)?;
    let client_auth = config
        .client_auth
        .as_ref()
        .filter(|ca| ca.enabled)
        .map(|ca| ClientAuthParams {
            ca_cert_path: ca.ca_cert_path.clone(),
            require_client_cert: ca.require_client_cert,
            pinned_certs: ca.pinned_certs.clone(),
            crl_path: ca.crl_path.clone(),
        });

    Ok(ServerConfigParams {
        cert_path,
        key_path,
        client_auth,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::schema::{GatewayClientAuthConfig, GatewayTlsConfig};

    /// Ensure the rustls `CryptoProvider` is installed (idempotent).
    fn ensure_crypto_provider() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    fn test_ca() -> (String, String, rcgen::KeyPair) {
        let ca_key = rcgen::KeyPair::generate().unwrap();
        let mut ca_params = rcgen::CertificateParams::new(vec!["Test CA".into()]).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();
        (ca_cert.pem(), ca_key.serialize_pem(), ca_key)
    }

    fn test_server_cert(ca_cert_pem: &str, ca_key: &rcgen::KeyPair) -> (String, String) {
        // Re-parse the CA cert for signing.
        let ca_key_clone = rcgen::KeyPair::from_pem(&ca_key.serialize_pem()).unwrap();
        let mut ca_params = rcgen::CertificateParams::new(vec!["Test CA".into()]).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca = ca_params.self_signed(&ca_key_clone).unwrap();

        let mut server_params = rcgen::CertificateParams::new(vec!["localhost".into()]).unwrap();
        server_params.is_ca = rcgen::IsCa::NoCa;
        let server_key = rcgen::KeyPair::generate().unwrap();
        let server_cert = server_params
            .signed_by(&server_key, &ca, &ca_key_clone)
            .unwrap();
        let _ = ca_cert_pem;
        (server_cert.pem(), server_key.serialize_pem())
    }

    fn write_temp_file(content: &str) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn test_build_server_config_no_client_auth() {
        ensure_crypto_provider();
        let (ca_cert_pem, _ca_key_pem, ca_key) = test_ca();
        let (server_cert_pem, server_key_pem) = test_server_cert(&ca_cert_pem, &ca_key);

        let cert_file = write_temp_file(&server_cert_pem);
        let key_file = write_temp_file(&server_key_pem);

        let tls_config = GatewayTlsConfig {
            enabled: true,
            cert_path: cert_file.path().to_str().unwrap().to_string(),
            key_path: key_file.path().to_str().unwrap().to_string(),
            client_auth: None,
        };

        // Should build successfully without client auth.
        let _server_config = build_server_config(&tls_config, Path::new("/nonexistent")).unwrap();
    }

    #[test]
    fn test_build_server_config_with_client_auth() {
        ensure_crypto_provider();
        let (ca_cert_pem, _ca_key_pem, ca_key) = test_ca();
        let (server_cert_pem, server_key_pem) = test_server_cert(&ca_cert_pem, &ca_key);

        let cert_file = write_temp_file(&server_cert_pem);
        let key_file = write_temp_file(&server_key_pem);
        let ca_file = write_temp_file(&ca_cert_pem);

        let tls_config = GatewayTlsConfig {
            enabled: true,
            cert_path: cert_file.path().to_str().unwrap().to_string(),
            key_path: key_file.path().to_str().unwrap().to_string(),
            client_auth: Some(GatewayClientAuthConfig {
                enabled: true,
                ca_cert_path: ca_file.path().to_str().unwrap().to_string(),
                require_client_cert: true,
                pinned_certs: vec![],
                crl_path: String::new(),
            }),
        };

        // Should build successfully with mandatory client auth.
        let _server_config = build_server_config(&tls_config, Path::new("/nonexistent")).unwrap();
    }

    #[test]
    fn test_build_server_config_client_auth_optional() {
        ensure_crypto_provider();
        let (ca_cert_pem, _ca_key_pem, ca_key) = test_ca();
        let (server_cert_pem, server_key_pem) = test_server_cert(&ca_cert_pem, &ca_key);

        let cert_file = write_temp_file(&server_cert_pem);
        let key_file = write_temp_file(&server_key_pem);
        let ca_file = write_temp_file(&ca_cert_pem);

        let tls_config = GatewayTlsConfig {
            enabled: true,
            cert_path: cert_file.path().to_str().unwrap().to_string(),
            key_path: key_file.path().to_str().unwrap().to_string(),
            client_auth: Some(GatewayClientAuthConfig {
                enabled: true,
                ca_cert_path: ca_file.path().to_str().unwrap().to_string(),
                require_client_cert: false,
                pinned_certs: vec![],
                crl_path: String::new(),
            }),
        };

        // Should build successfully with optional client auth.
        let _server_config = build_server_config(&tls_config, Path::new("/nonexistent")).unwrap();
    }

    #[test]
    fn test_config_defaults_deserialization() {
        let toml_str = r#"
            cert_path = "/tmp/cert.pem"
            key_path = "/tmp/key.pem"
        "#;
        let config: GatewayTlsConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.enabled);
        assert!(config.client_auth.is_none());
    }

    #[test]
    fn test_client_auth_config_defaults() {
        let toml_str = r#"
            ca_cert_path = "/tmp/ca.pem"
        "#;
        let config: GatewayClientAuthConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.enabled);
        assert!(config.require_client_cert);
        assert!(config.pinned_certs.is_empty());
    }

    #[test]
    fn test_build_server_config_with_pinning() {
        ensure_crypto_provider();
        let (ca_cert_pem, _ca_key_pem, ca_key) = test_ca();
        let (server_cert_pem, server_key_pem) = test_server_cert(&ca_cert_pem, &ca_key);

        let cert_file = write_temp_file(&server_cert_pem);
        let key_file = write_temp_file(&server_key_pem);
        let ca_file = write_temp_file(&ca_cert_pem);

        let tls_config = GatewayTlsConfig {
            enabled: true,
            cert_path: cert_file.path().to_str().unwrap().to_string(),
            key_path: key_file.path().to_str().unwrap().to_string(),
            client_auth: Some(GatewayClientAuthConfig {
                enabled: true,
                ca_cert_path: ca_file.path().to_str().unwrap().to_string(),
                require_client_cert: true,
                pinned_certs: vec!["aabbccdd".to_string()],
                crl_path: String::new(),
            }),
        };

        // Should build successfully - pinning is checked at connection time, not config time.
        let _server_config = build_server_config(&tls_config, Path::new("/nonexistent")).unwrap();
    }

    #[test]
    fn test_disabled_client_auth_skipped() {
        ensure_crypto_provider();
        let (ca_cert_pem, _ca_key_pem, ca_key) = test_ca();
        let (server_cert_pem, server_key_pem) = test_server_cert(&ca_cert_pem, &ca_key);

        let cert_file = write_temp_file(&server_cert_pem);
        let key_file = write_temp_file(&server_key_pem);

        // client_auth present but enabled=false should be treated as no client auth.
        let tls_config = GatewayTlsConfig {
            enabled: true,
            cert_path: cert_file.path().to_str().unwrap().to_string(),
            key_path: key_file.path().to_str().unwrap().to_string(),
            client_auth: Some(GatewayClientAuthConfig {
                enabled: false,
                ca_cert_path: "/nonexistent".to_string(),
                require_client_cert: true,
                pinned_certs: vec![],
                crl_path: String::new(),
            }),
        };

        // Should succeed because client_auth.enabled=false skips the CA loading.
        let _server_config = build_server_config(&tls_config, Path::new("/nonexistent")).unwrap();
    }

    #[test]
    fn auto_cert_is_provisioned_once_and_reused_across_restarts() {
        ensure_crypto_provider();
        let data_dir = tempfile::tempdir().unwrap();
        let tls_config = GatewayTlsConfig {
            enabled: true,
            ..GatewayTlsConfig::default()
        };

        assert!(served_cert_fingerprint(&tls_config, data_dir.path()).is_none());
        build_tls_acceptor(&tls_config, data_dir.path()).unwrap();
        let first = served_cert_fingerprint(&tls_config, data_dir.path()).unwrap();
        assert_eq!(first.len(), 64);

        // A second start (new acceptor) must serve the same certificate, or every
        // paired phone's pin would break on reboot.
        build_tls_acceptor(&tls_config, data_dir.path()).unwrap();
        assert_eq!(
            served_cert_fingerprint(&tls_config, data_dir.path()).unwrap(),
            first
        );
    }
}
