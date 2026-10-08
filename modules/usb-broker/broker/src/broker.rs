//! The broker proper: owns the policy and the per-device state, talks to sysfs.
use crate::policy::{self, Policy, UsbDevice, Verdict};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct Record {
    pub device: UsbDevice,
    pub verdict: Verdict,
    pub reason: String,
    pub seen_at: String,
    /// read-only mount points created for this device's partitions
    pub mounts: Vec<String>,
}

pub struct Broker {
    /// `/sys` normally; a fixture tree in tests.
    pub sysfs: PathBuf,
    pub policy_path: PathBuf,
    pub policy: Policy,
    pub audit_dir: PathBuf,
    pub media_dir: PathBuf,
    /// Perform real `mount(8)` calls (root only). Off in tests and on L0.
    pub mount_enabled: bool,
    pub devices: BTreeMap<String, Record>,
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl Broker {
    pub fn new(
        sysfs: PathBuf,
        policy_path: PathBuf,
        audit_dir: PathBuf,
        media_dir: PathBuf,
        mount_enabled: bool,
    ) -> Result<Self> {
        let policy = policy::load_policy(&policy_path)?;
        fs::create_dir_all(&audit_dir)?;
        Ok(Self {
            sysfs,
            policy_path,
            policy,
            audit_dir,
            media_dir,
            mount_enabled,
            devices: BTreeMap::new(),
        })
    }

    fn usb_devices_dir(&self) -> PathBuf {
        self.sysfs.join("bus/usb/devices")
    }

    pub fn audit(&self, event: &str, dev: Option<&UsbDevice>, detail: &str) {
        let line = serde_json::json!({
            "ts": now(), "event": event, "detail": detail,
            "device": dev.map(|d| serde_json::json!({
                "sysname": d.sysname, "id": d.identity(), "product": d.product, "manufacturer": d.manufacturer,
                "classes": d.interfaces.iter().map(|i| i.class).collect::<Vec<_>>()})),
        });
        let path = self.audit_dir.join(format!(
            "usb-{}.jsonl",
            chrono::Utc::now().format("%Y-%m-%d")
        ));
        if let Err(e) = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .and_then(|mut f| writeln!(f, "{line}"))
        {
            tracing::error!(%e, "usb audit write failed");
        }
        tracing::info!(event, detail, "usb");
    }

    /// Every host controller: new devices arrive unauthorized. Devices already present
    /// keep their state (the kernel bound them at boot).
    pub fn enforce_default_deny(&self) -> Result<usize> {
        let mut n = 0;
        for e in fs::read_dir(self.usb_devices_dir())
            .context("sysfs usb devices")?
            .flatten()
        {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("usb") {
                let p = e.path().join("authorized_default");
                if p.exists() {
                    fs::write(&p, "0").with_context(|| format!("write {}", p.display()))?;
                    n += 1;
                }
            }
        }
        self.audit("default-deny", None, &format!("{n} host controllers"));
        Ok(n)
    }

    fn is_device_name(name: &str) -> bool {
        // `1-2`, `2-1.3` are devices; `usb1` root hubs and `1-2:1.0` interfaces are not
        name.contains('-') && !name.contains(':')
    }

    /// Classify every device currently present; boot devices stay as the kernel left them
    /// when `trust_boot_devices` is set.
    pub fn scan(&mut self) -> Result<()> {
        for e in fs::read_dir(self.usb_devices_dir())?.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if Self::is_device_name(&name) && !self.devices.contains_key(&name) {
                self.handle_add(&e.path(), true)?;
            }
        }
        Ok(())
    }

    fn write_authorized(&self, dev: &UsbDevice, on: bool) -> Result<()> {
        let p = self.usb_devices_dir().join(&dev.sysname).join("authorized");
        fs::write(&p, if on { "1" } else { "0" }).with_context(|| format!("write {}", p.display()))
    }

    /// A usb_device appeared (or was present at scan). Returns its verdict.
    pub fn handle_add(&mut self, dev_dir: &Path, at_scan: bool) -> Result<Verdict> {
        let dev = policy::read_device(dev_dir)?;
        let (verdict, reason) = if at_scan && dev.authorized && self.policy.trust_boot_devices {
            (Verdict::Allow, "present at boot".to_string())
        } else {
            let v = policy::decide(&self.policy, &dev);
            let r = match v {
                Verdict::Reject if dev.is_storage() && dev.has_class(policy::CLASS_HID) => {
                    "badusb: storage + HID on one device"
                }
                Verdict::Reject => "deny list",
                Verdict::Allow if dev.is_hub() => "hub",
                Verdict::Allow => "allow list",
                Verdict::Pending => "awaiting user approval",
            };
            (v, r.to_string())
        };
        match verdict {
            Verdict::Allow => {
                if !dev.authorized {
                    self.write_authorized(&dev, true)?;
                }
            }
            Verdict::Pending | Verdict::Reject => {
                if dev.authorized && !(at_scan && self.policy.trust_boot_devices) {
                    self.write_authorized(&dev, false)?;
                }
            }
        }
        self.audit(
            match verdict {
                Verdict::Allow => "allow",
                Verdict::Pending => "pending",
                Verdict::Reject => "reject",
            },
            Some(&dev),
            &reason,
        );
        let mut dev = dev;
        dev.authorized = matches!(verdict, Verdict::Allow);
        self.devices.insert(
            dev.sysname.clone(),
            Record {
                device: dev,
                verdict,
                reason,
                seen_at: now(),
                mounts: Vec::new(),
            },
        );
        Ok(verdict)
    }

    pub fn handle_remove(&mut self, sysname: &str) {
        if let Some(rec) = self.devices.remove(sysname) {
            for m in &rec.mounts {
                self.umount(m);
            }
            self.audit("remove", Some(&rec.device), "");
        }
    }

    /// User decision from the API. `remember` persists the identity in the policy.
    pub fn approve(&mut self, sysname: &str, remember: bool) -> Result<bool> {
        let Some(rec) = self.devices.get(sysname).cloned() else {
            return Ok(false);
        };
        if rec.verdict == Verdict::Reject && rec.reason.starts_with("badusb") {
            anyhow::bail!("a storage device with a HID interface cannot be approved");
        }
        self.write_authorized(&rec.device, true)?;
        if remember {
            let id = rec.device.identity();
            if !self.policy.allow.contains(&id) {
                self.policy.allow.push(id);
                self.policy.deny.retain(|d| d != &rec.device.identity());
                policy::save_policy(&self.policy_path, &self.policy)?;
            }
        }
        self.audit(
            "approve",
            Some(&rec.device),
            if remember { "remembered" } else { "once" },
        );
        if let Some(r) = self.devices.get_mut(sysname) {
            r.verdict = Verdict::Allow;
            r.reason = "approved by user".into();
            r.device.authorized = true;
        }
        Ok(true)
    }

    pub fn deny(&mut self, sysname: &str, remember: bool) -> Result<bool> {
        let Some(rec) = self.devices.get(sysname).cloned() else {
            return Ok(false);
        };
        self.write_authorized(&rec.device, false)?;
        for m in &rec.mounts {
            self.umount(m);
        }
        if remember {
            let id = rec.device.identity();
            if !self.policy.deny.contains(&id) {
                self.policy.deny.push(id.clone());
                self.policy.allow.retain(|a| a != &id);
                policy::save_policy(&self.policy_path, &self.policy)?;
            }
        }
        self.audit(
            "deny",
            Some(&rec.device),
            if remember { "remembered" } else { "once" },
        );
        if let Some(r) = self.devices.get_mut(sysname) {
            r.verdict = Verdict::Reject;
            r.reason = "denied by user".into();
            r.device.authorized = false;
            r.mounts.clear();
        }
        Ok(true)
    }

    /// A block device from an approved USB device: mount it read-only, no exec/suid/dev.
    /// `usb_sysname` is the usb_device ancestor (`1-2`), `devnode` e.g. `/dev/sda1`.
    pub fn handle_block_add(
        &mut self,
        usb_sysname: &str,
        devnode: &str,
        label: Option<&str>,
    ) -> Result<Option<String>> {
        let Some(rec) = self.devices.get(usb_sysname) else {
            return Ok(None);
        };
        if rec.verdict != Verdict::Allow {
            self.audit(
                "block-ignored",
                Some(&rec.device),
                &format!("{devnode}: device not approved"),
            );
            return Ok(None);
        }
        let base = devnode.rsplit('/').next().unwrap_or(devnode);
        let name = label
            .filter(|l| {
                !l.is_empty()
                    && l.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            })
            .unwrap_or(base);
        let target = self.media_dir.join(name);
        if self.mount_enabled {
            fs::create_dir_all(&target)?;
            let st = Command::new("mount")
                .args(["-o", "ro,nosuid,nodev,noexec", devnode])
                .arg(&target)
                .status()
                .context("mount")?;
            if !st.success() {
                self.audit("mount-failed", Some(&rec.device), devnode);
                return Ok(None);
            }
        }
        let t = target.to_string_lossy().into_owned();
        self.audit(
            "mount",
            Some(&rec.device),
            &format!("{devnode} -> {t} (ro,nosuid,nodev,noexec)"),
        );
        if let Some(r) = self.devices.get_mut(usb_sysname) {
            r.mounts.push(t.clone());
        }
        Ok(Some(t))
    }

    fn umount(&self, target: &str) {
        if self.mount_enabled {
            let _ = Command::new("umount").arg("-l").arg(target).status();
            let _ = fs::remove_dir(target);
        }
        self.audit("umount", None, target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(root: &Path, name: &str, vid: &str, pid: &str, classes: &[u8], authorized: bool) {
        let d = root.join("bus/usb/devices").join(name);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("idVendor"), vid).unwrap();
        fs::write(d.join("idProduct"), pid).unwrap();
        fs::write(d.join("authorized"), if authorized { "1" } else { "0" }).unwrap();
        for (i, c) in classes.iter().enumerate() {
            let idir = d.join(format!("{name}:1.{i}"));
            fs::create_dir_all(&idir).unwrap();
            fs::write(idir.join("bInterfaceClass"), format!("{c:02x}")).unwrap();
            fs::write(idir.join("bInterfaceSubClass"), "00").unwrap();
            fs::write(idir.join("bInterfaceProtocol"), "00").unwrap();
        }
    }

    fn authorized(root: &Path, name: &str) -> String {
        fs::read_to_string(root.join("bus/usb/devices").join(name).join("authorized")).unwrap()
    }

    fn broker(root: &Path) -> Broker {
        fs::create_dir_all(root.join("bus/usb/devices/usb1")).unwrap();
        fs::write(root.join("bus/usb/devices/usb1/authorized_default"), "1").unwrap();
        Broker::new(
            root.to_path_buf(),
            root.join("state/policy.json"),
            root.join("audit"),
            root.join("media"),
            false,
        )
        .unwrap()
    }

    #[test]
    fn default_deny_and_boot_trust() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        fixture(root, "1-1", "046d", "c31c", &[0x03], true); // keyboard present at boot
        let mut b = broker(root);
        assert_eq!(b.enforce_default_deny().unwrap(), 1);
        assert_eq!(
            fs::read_to_string(root.join("bus/usb/devices/usb1/authorized_default")).unwrap(),
            "0"
        );
        b.scan().unwrap();
        assert_eq!(b.devices["1-1"].verdict, Verdict::Allow);
        assert_eq!(authorized(root, "1-1"), "1");
    }

    #[test]
    fn hotplug_keyboard_pends_until_approved_and_remembered() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let mut b = broker(root);
        fixture(root, "1-2", "046d", "c31c", &[0x03], false);
        let v = b
            .handle_add(&root.join("bus/usb/devices/1-2"), false)
            .unwrap();
        assert_eq!(v, Verdict::Pending);
        assert_eq!(authorized(root, "1-2"), "0");
        assert!(b.approve("1-2", true).unwrap());
        assert_eq!(authorized(root, "1-2"), "1");
        assert!(
            policy::load_policy(&root.join("state/policy.json"))
                .unwrap()
                .allow
                .contains(&"046d:c31c:".to_string())
        );
        // replug: now allowed straight away
        b.handle_remove("1-2");
        fixture(root, "1-2", "046d", "c31c", &[0x03], false);
        assert_eq!(
            b.handle_add(&root.join("bus/usb/devices/1-2"), false)
                .unwrap(),
            Verdict::Allow
        );
        assert_eq!(authorized(root, "1-2"), "1");
    }

    #[test]
    fn badusb_rejected_and_not_approvable() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let mut b = broker(root);
        fixture(root, "1-3", "1234", "5678", &[0x08, 0x03], false);
        assert_eq!(
            b.handle_add(&root.join("bus/usb/devices/1-3"), false)
                .unwrap(),
            Verdict::Reject
        );
        assert_eq!(authorized(root, "1-3"), "0");
        assert!(b.approve("1-3", true).is_err());
        assert_eq!(authorized(root, "1-3"), "0");
        assert!(
            b.handle_block_add("1-3", "/dev/sdb1", None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn storage_mounts_only_after_approval() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let mut b = broker(root);
        fixture(root, "1-4", "0781", "5567", &[0x08], false);
        b.handle_add(&root.join("bus/usb/devices/1-4"), false)
            .unwrap();
        assert!(
            b.handle_block_add("1-4", "/dev/sda1", Some("MYSTICK"))
                .unwrap()
                .is_none()
        );
        b.approve("1-4", false).unwrap();
        let m = b
            .handle_block_add("1-4", "/dev/sda1", Some("MYSTICK"))
            .unwrap()
            .unwrap();
        assert!(m.ends_with("/media/MYSTICK"), "{m}");
        // a label with path tricks falls back to the device node name
        let m2 = b
            .handle_block_add("1-4", "/dev/sda2", Some("../etc"))
            .unwrap()
            .unwrap();
        assert!(m2.ends_with("/media/sda2"), "{m2}");
        b.deny("1-4", true).unwrap();
        assert_eq!(authorized(root, "1-4"), "0");
        assert!(b.devices["1-4"].mounts.is_empty());
        let audit = fs::read_dir(root.join("audit"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let text = fs::read_to_string(audit).unwrap();
        assert!(
            text.contains("\"event\":\"mount\"")
                && text.contains("\"event\":\"umount\"")
                && text.contains("\"event\":\"deny\"")
        );
    }
}
