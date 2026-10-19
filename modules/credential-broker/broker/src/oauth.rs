//! OAuth 2.0 refresh for stored credentials.
//!
//! A credential whose stored value is a JSON object with `"type": "oauth2"` is
//! refreshed by the broker (RFC 6749 §6) instead of being sent as-is:
//!
//! ```json
//! {"type": "oauth2", "token_url": "https://oauth2.googleapis.com/token",
//!  "client_id": "...", "client_secret": "...", "refresh_token": "..."}
//! ```
//!
//! The access token and its expiry are written back into the encrypted store, as
//! is a rotated refresh token when the provider returns one. Any other value is a
//! static token, unchanged from before.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// Refresh this many seconds before the provider's stated expiry.
const SKEW_SECS: u64 = 60;

// No Debug: the value holds the refresh token and client secret.
#[derive(Clone, Serialize, Deserialize)]
pub struct OAuthCredential {
    #[serde(rename = "type")]
    pub kind: String,
    pub token_url: String,
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    pub refresh_token: String,
    /// Optional `scope` parameter sent with the refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default)]
    pub access_token: String,
    /// Unix seconds; 0 = unknown, refresh before first use.
    #[serde(default)]
    pub expires_at: u64,
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    refresh_token: Option<String>,
}

impl OAuthCredential {
    /// `Some` only for a JSON object tagged `"type": "oauth2"`.
    pub fn parse(value: &str) -> Option<Self> {
        if !value.trim_start().starts_with('{') {
            return None;
        }
        serde_json::from_str::<Self>(value)
            .ok()
            .filter(|c| c.kind == "oauth2")
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn needs_refresh(&self, now: u64) -> bool {
        self.access_token.is_empty() || self.expires_at <= now + SKEW_SECS
    }

    /// Exchange the refresh token for a new access token.
    pub async fn refresh(&mut self, http: &reqwest::Client, now: u64) -> Result<()> {
        if !self.token_url.starts_with("https://") {
            bail!("token_url must be https://");
        }
        let mut form = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", self.refresh_token.as_str()),
            ("client_id", self.client_id.as_str()),
        ];
        if let Some(secret) = &self.client_secret {
            form.push(("client_secret", secret.as_str()));
        }
        if let Some(scope) = &self.scope {
            form.push(("scope", scope.as_str()));
        }
        let resp = http
            .post(&self.token_url)
            .header("accept", "application/json")
            .form(&form)
            .send()
            .await
            .context("token endpoint")?;
        let status = resp.status();
        if !status.is_success() {
            // The body may echo the request; keep it out of logs and replies.
            bail!("token endpoint answered {status}");
        }
        let reply: TokenReply = resp.json().await.context("token reply")?;
        if reply.access_token.is_empty() {
            bail!("token endpoint returned no access_token");
        }
        self.access_token = reply.access_token;
        // No expires_in: assume an hour, the common default, and let a 401 force
        // an early refresh.
        self.expires_at = now + reply.expires_in.unwrap_or(3600);
        if let Some(rt) = reply.refresh_token.filter(|t| !t.is_empty()) {
            self.refresh_token = rt;
        }
        Ok(())
    }
}
