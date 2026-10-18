//! Credential store (ChaCha20-Poly1305 under a 32-byte key file), grants and approvals.
//!
//! Same algorithm as the agent's secrets store (`zeroclaw-config/src/secrets.rs`),
//! reimplemented here on purpose: that crate pulls postgres, rusqlite, websockets
//! and the whole agent schema, which is far too much dependency surface for a
//! process whose only job is to hold tokens. Format: `enc:v1:<hex nonce>:<hex ct>`.
use anyhow::{Context, Result, bail};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SecretBox {
    key: [u8; 32],
}

impl SecretBox {
    /// Load the key file, creating it (0600) when absent.
    pub fn open(key_path: &Path) -> Result<Self> {
        let mut key = [0u8; 32];
        match fs::read(key_path) {
            Ok(bytes) if bytes.len() == 32 => key.copy_from_slice(&bytes),
            Ok(_) => bail!("{}: key file is not 32 bytes", key_path.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                rand::rngs::OsRng.fill_bytes(&mut key);
                if let Some(parent) = key_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut f = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(key_path)
                    .with_context(|| format!("create key {}", key_path.display()))?;
                f.write_all(&key)?;
            }
            Err(e) => return Err(e).with_context(|| format!("read key {}", key_path.display())),
        }
        let mode = fs::metadata(key_path)?.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            bail!(
                "{}: key file mode {mode:o} is group/world accessible",
                key_path.display()
            );
        }
        Ok(Self { key })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let mut nonce = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ct = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|_| anyhow::anyhow!("encrypt failed"))?;
        Ok(format!("enc:v1:{}:{}", hex::encode(nonce), hex::encode(ct)))
    }

    pub fn decrypt(&self, value: &str) -> Result<String> {
        let mut parts = value.splitn(4, ':');
        let (Some("enc"), Some("v1"), Some(nonce_hex), Some(ct_hex)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            bail!("not an enc:v1 value");
        };
        let nonce = hex::decode(nonce_hex).context("nonce hex")?;
        let ct = hex::decode(ct_hex).context("ciphertext hex")?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let pt = cipher
            .decrypt(Nonce::from_slice(&nonce), ct.as_ref())
            .map_err(|_| anyhow::anyhow!("decrypt failed (wrong key or tampered)"))?;
        Ok(String::from_utf8(pt).context("plaintext utf8")?)
    }
}

impl Drop for SecretBox {
    fn drop(&mut self) {
        self.key.fill(0);
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Persisted {
    /// credential name -> `enc:v1:...`
    credentials: BTreeMap<String, String>,
    grants: Vec<Grant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Grant {
    pub task: String,
    pub scope: String,
    pub expires_at: u64,
    pub granted_by: String,
}

/// Everything the broker persists under `state_dir`.
pub struct Store {
    secret: SecretBox,
    path: PathBuf,
    data: Persisted,
    /// (task, request hash) pairs the user approved; consumed on use.
    approvals: Vec<(String, String)>,
}

impl Store {
    pub fn open(state_dir: &Path) -> Result<Self> {
        fs::create_dir_all(state_dir)?;
        let secret = SecretBox::open(&state_dir.join(".secret_key"))?;
        let path = state_dir.join("store.json");
        let data = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).context("parse store.json")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Persisted::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            secret,
            path,
            data,
            approvals: Vec::new(),
        })
    }

    fn save(&self) -> Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(&serde_json::to_vec_pretty(&self.data)?)?;
        f.sync_all()?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn set_credential(&mut self, name: &str, value: &str) -> Result<()> {
        let enc = self.secret.encrypt(value)?;
        self.data.credentials.insert(name.to_string(), enc);
        self.save()
    }

    pub fn delete_credential(&mut self, name: &str) -> Result<bool> {
        let existed = self.data.credentials.remove(name).is_some();
        self.save()?;
        Ok(existed)
    }

    pub fn credential(&self, name: &str) -> Result<Option<String>> {
        match self.data.credentials.get(name) {
            Some(enc) => Ok(Some(self.secret.decrypt(enc)?)),
            None => Ok(None),
        }
    }

    pub fn credential_names(&self) -> Vec<String> {
        self.data.credentials.keys().cloned().collect()
    }

    pub fn grant(
        &mut self,
        task: &str,
        scopes: &[String],
        ttl_secs: u64,
        by: &str,
    ) -> Result<Vec<Grant>> {
        let expires_at = now_secs() + ttl_secs;
        self.data.grants.retain(|g| {
            g.expires_at > now_secs() && !(g.task == task && scopes.contains(&g.scope))
        });
        let mut new = Vec::new();
        for s in scopes {
            let g = Grant {
                task: task.into(),
                scope: s.clone(),
                expires_at,
                granted_by: by.into(),
            };
            self.data.grants.push(g.clone());
            new.push(g);
        }
        self.save()?;
        Ok(new)
    }

    pub fn revoke_task(&mut self, task: &str) -> Result<usize> {
        let before = self.data.grants.len();
        self.data.grants.retain(|g| g.task != task);
        self.approvals.retain(|(t, _)| t != task);
        self.save()?;
        Ok(before - self.data.grants.len())
    }

    pub fn has_grant(&self, task: &str, scope: &str) -> bool {
        let now = now_secs();
        self.data
            .grants
            .iter()
            .any(|g| g.task == task && g.scope == scope && g.expires_at > now)
    }

    pub fn grants(&self) -> Vec<Grant> {
        let now = now_secs();
        self.data
            .grants
            .iter()
            .filter(|g| g.expires_at > now)
            .cloned()
            .collect()
    }

    pub fn approve(&mut self, task: &str, request_hash: &str) {
        if !self
            .approvals
            .iter()
            .any(|(t, h)| t == task && h == request_hash)
        {
            self.approvals.push((task.into(), request_hash.into()));
        }
    }

    /// Consume an approval. Approvals are single-use and bound to the exact request hash.
    pub fn take_approval(&mut self, task: &str, request_hash: &str) -> bool {
        if let Some(i) = self
            .approvals
            .iter()
            .position(|(t, h)| t == task && h == request_hash)
        {
            self.approvals.remove(i);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tamper() {
        let dir = tempfile::tempdir().unwrap();
        let sb = SecretBox::open(&dir.path().join("k")).unwrap();
        let enc = sb.encrypt("tok-123").unwrap();
        assert!(enc.starts_with("enc:v1:"));
        assert_eq!(sb.decrypt(&enc).unwrap(), "tok-123");
        let mut bad = enc.clone();
        bad.pop();
        bad.push(if enc.ends_with('0') { '1' } else { '0' });
        assert!(sb.decrypt(&bad).is_err());
        // a second open reads the same key
        let sb2 = SecretBox::open(&dir.path().join("k")).unwrap();
        assert_eq!(sb2.decrypt(&enc).unwrap(), "tok-123");
    }

    #[test]
    fn key_file_mode_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("k");
        SecretBox::open(&p).unwrap();
        assert_eq!(
            fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(SecretBox::open(&p).is_err());
    }

    #[test]
    fn store_persists_credentials_and_grants() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut s = Store::open(dir.path()).unwrap();
            s.set_credential("google.user", "ya29.x").unwrap();
            s.grant("t-1", &["google.gmail.read".into()], 60, "test")
                .unwrap();
        }
        let s = Store::open(dir.path()).unwrap();
        assert_eq!(
            s.credential("google.user").unwrap().as_deref(),
            Some("ya29.x")
        );
        assert!(s.has_grant("t-1", "google.gmail.read"));
        assert!(!s.has_grant("t-1", "google.gmail.send"));
        assert!(!s.has_grant("t-2", "google.gmail.read"));
        // the on-disk file never holds the plaintext
        let raw = fs::read_to_string(dir.path().join("store.json")).unwrap();
        assert!(!raw.contains("ya29.x"));
    }

    #[test]
    fn approvals_are_single_use() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::open(dir.path()).unwrap();
        s.approve("t-1", "abc");
        assert!(s.take_approval("t-1", "abc"));
        assert!(!s.take_approval("t-1", "abc"));
        assert!(!s.take_approval("t-1", "abd"));
    }
}
