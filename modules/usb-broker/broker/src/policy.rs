//! USB device model read from sysfs, and the admission policy.
//!
//! The kernel's own authorization is the enforcement point: with
//! `/sys/bus/usb/devices/usbN/authorized_default = 0` every newly attached device
//! arrives with `authorized = 0` (no driver binds, no interface is exposed) until
//! the broker writes `authorized = 1`. This is what USBGuard does underneath; we do
//! not need the daemon or its rule language for the policy below.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const CLASS_HID: u8 = 0x03;
pub const CLASS_MASS_STORAGE: u8 = 0x08;
pub const CLASS_HUB: u8 = 0x09;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Interface {
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsbDevice {
    /// sysfs name, e.g. `1-2` or `2-1.3`
    pub sysname: String,
    pub vendor_id: String,
    pub product_id: String,
    pub serial: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub interfaces: Vec<Interface>,
    pub authorized: bool,
}

impl UsbDevice {
    /// Stable identity used for the allow-list: vendor:product:serial (serial may be absent).
    pub fn identity(&self) -> String {
        format!(
            "{}:{}:{}",
            self.vendor_id,
            self.product_id,
            self.serial.clone().unwrap_or_default()
        )
    }
    pub fn has_class(&self, class: u8) -> bool {
        self.interfaces.iter().any(|i| i.class == class)
    }
    pub fn is_storage(&self) -> bool {
        self.has_class(CLASS_MASS_STORAGE)
    }
    pub fn is_hub(&self) -> bool {
        self.has_class(CLASS_HUB) && self.interfaces.iter().all(|i| i.class == CLASS_HUB)
    }
}

fn read_trim(p: &Path) -> Option<String> {
    fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_hex_u8(p: &Path) -> Option<u8> {
    read_trim(p).and_then(|s| u8::from_str_radix(&s, 16).ok())
}

/// Read one usb_device node (`<sysfs>/bus/usb/devices/<name>`) including the interfaces
/// the kernel has enumerated under it (`<name>:<config>.<iface>`). Interfaces are only
/// present once the device is authorized; for an unauthorized device we fall back to the
/// device-level class and the raw `descriptors` blob.
pub fn read_device(dev_dir: &Path) -> Result<UsbDevice> {
    let sysname = dev_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?")
        .to_string();
    let vendor_id =
        read_trim(&dev_dir.join("idVendor")).with_context(|| format!("{sysname}: idVendor"))?;
    let product_id =
        read_trim(&dev_dir.join("idProduct")).with_context(|| format!("{sysname}: idProduct"))?;
    let mut interfaces = Vec::new();
    if let Ok(rd) = fs::read_dir(dev_dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("{sysname}:")) {
                let p = e.path();
                if let (Some(class), Some(subclass), Some(protocol)) = (
                    read_hex_u8(&p.join("bInterfaceClass")),
                    read_hex_u8(&p.join("bInterfaceSubClass")),
                    read_hex_u8(&p.join("bInterfaceProtocol")),
                ) {
                    interfaces.push(Interface {
                        class,
                        subclass,
                        protocol,
                    });
                }
            }
        }
    }
    if interfaces.is_empty() {
        interfaces = interfaces_from_descriptors(&dev_dir.join("descriptors"));
    }
    interfaces.sort_by_key(|i| (i.class, i.subclass, i.protocol));
    interfaces.dedup();
    Ok(UsbDevice {
        sysname,
        vendor_id,
        product_id,
        serial: read_trim(&dev_dir.join("serial")),
        manufacturer: read_trim(&dev_dir.join("manufacturer")),
        product: read_trim(&dev_dir.join("product")),
        interfaces,
        authorized: read_trim(&dev_dir.join("authorized")).as_deref() == Some("1"),
    })
}

/// Parse the raw descriptor blob (device descriptor followed by configuration
/// descriptors) and collect every interface descriptor (type 0x04). This is what lets
/// us classify a device the kernel has NOT yet authorized -- the blob is readable
/// regardless, so a BadUSB stick is caught before any driver binds.
pub fn interfaces_from_descriptors(path: &Path) -> Vec<Interface> {
    let Ok(blob) = fs::read(path) else {
        return Vec::new();
    };
    parse_descriptor_blob(&blob)
}

pub fn parse_descriptor_blob(blob: &[u8]) -> Vec<Interface> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 2 <= blob.len() {
        let len = blob[i] as usize;
        if len < 2 || i + len > blob.len() {
            break;
        }
        if blob[i + 1] == 0x04 && len >= 9 {
            out.push(Interface {
                class: blob[i + 5],
                subclass: blob[i + 6],
                protocol: blob[i + 7],
            });
        }
        i += len;
    }
    out
}

/// What the policy says about a freshly seen device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Allowed without asking (hubs, or a device the user already approved).
    Allow,
    /// Needs the user's approval before the kernel may bind drivers.
    Pending,
    /// Never allowed: a storage device that also presents a HID interface
    /// (a stick that types), or anything on the deny list.
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Policy {
    /// identities (vendor:product:serial) the user approved; `vendor:product:` matches any serial
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    /// Devices present at boot stay authorized (the kernel already bound them); default true.
    #[serde(default = "default_true")]
    pub trust_boot_devices: bool,
}
fn default_true() -> bool {
    true
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            allow: Vec::new(),
            deny: Vec::new(),
            trust_boot_devices: true,
        }
    }
}

fn list_matches(list: &[String], dev: &UsbDevice) -> bool {
    let id = dev.identity();
    let any_serial = format!("{}:{}:", dev.vendor_id, dev.product_id);
    list.iter().any(|e| e == &id || e == &any_serial)
}

pub fn decide(policy: &Policy, dev: &UsbDevice) -> Verdict {
    if list_matches(&policy.deny, dev) {
        return Verdict::Reject;
    }
    // BadUSB: mass storage + HID on one device. A keyboard that is also a disk is the
    // classic attack, and no legitimate stick needs to type.
    if dev.is_storage() && dev.has_class(CLASS_HID) {
        return Verdict::Reject;
    }
    if dev.is_hub() {
        return Verdict::Allow;
    }
    if list_matches(&policy.allow, dev) {
        return Verdict::Allow;
    }
    Verdict::Pending
}

/// Where the policy file lives; on the device `/mnt/data/redrob/usb/policy.json`.
pub fn load_policy(path: &Path) -> Result<Policy> {
    match fs::read(path) {
        Ok(b) => Ok(serde_json::from_slice(&b).context("parse usb policy")?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Policy::default()),
        Err(e) => Err(e.into()),
    }
}

pub fn save_policy(path: &Path, policy: &Policy) -> Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    let tmp: PathBuf = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(policy)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(ifaces: &[(u8, u8, u8)]) -> UsbDevice {
        UsbDevice {
            sysname: "1-2".into(),
            vendor_id: "0781".into(),
            product_id: "5567".into(),
            serial: Some("4C53".into()),
            manufacturer: None,
            product: None,
            interfaces: ifaces
                .iter()
                .map(|&(c, s, p)| Interface {
                    class: c,
                    subclass: s,
                    protocol: p,
                })
                .collect(),
            authorized: false,
        }
    }

    #[test]
    fn badusb_is_rejected_even_if_allow_listed() {
        let mut p = Policy::default();
        p.allow.push("0781:5567:4C53".into());
        let stick_that_types = dev(&[(0x08, 0x06, 0x50), (0x03, 0x01, 0x01)]);
        assert_eq!(decide(&p, &stick_that_types), Verdict::Reject);
    }

    #[test]
    fn plain_storage_pends_then_allows_after_approval() {
        let mut p = Policy::default();
        let stick = dev(&[(0x08, 0x06, 0x50)]);
        assert_eq!(decide(&p, &stick), Verdict::Pending);
        p.allow.push(stick.identity());
        assert_eq!(decide(&p, &stick), Verdict::Allow);
        // a different serial of the same model is not covered by a serial-bound entry
        let mut other = stick.clone();
        other.serial = Some("FFFF".into());
        assert_eq!(decide(&p, &other), Verdict::Pending);
        // but a `vendor:product:` entry covers any serial
        p.allow.push("0781:5567:".into());
        assert_eq!(decide(&p, &other), Verdict::Allow);
    }

    #[test]
    fn keyboard_pends_hub_allows_deny_wins() {
        let p = Policy::default();
        assert_eq!(decide(&p, &dev(&[(0x03, 0x01, 0x01)])), Verdict::Pending);
        assert_eq!(decide(&p, &dev(&[(0x09, 0, 0)])), Verdict::Allow);
        let mut p2 = Policy::default();
        p2.allow.push("0781:5567:4C53".into());
        p2.deny.push("0781:5567:4C53".into());
        assert_eq!(decide(&p2, &dev(&[(0x08, 0x06, 0x50)])), Verdict::Reject);
    }

    #[test]
    fn descriptor_blob_yields_interfaces_without_authorization() {
        // device descriptor (18) + config (9) + interface (9, class 08) + interface (9, class 03)
        let mut blob = vec![
            18, 1, 0, 2, 0, 0, 0, 64, 0x81, 0x07, 0x67, 0x55, 0, 1, 1, 2, 3, 1,
        ];
        blob.extend([9, 2, 0x20, 0, 2, 1, 0, 0x80, 50]);
        blob.extend([9, 4, 0, 0, 2, 0x08, 0x06, 0x50, 0]);
        blob.extend([9, 4, 1, 0, 1, 0x03, 0x01, 0x01, 0]);
        let ifs = parse_descriptor_blob(&blob);
        assert_eq!(ifs.len(), 2);
        assert_eq!(ifs[0].class, 0x08);
        assert_eq!(ifs[1].class, 0x03);
        // truncated/garbage blob does not panic
        assert!(parse_descriptor_blob(&[9, 4, 0]).is_empty());
        assert!(parse_descriptor_blob(&[0, 0, 0]).is_empty());
    }

    #[test]
    fn read_device_from_sysfs_fixture() {
        let d = tempfile::tempdir().unwrap();
        let dev_dir = d.path().join("1-3");
        fs::create_dir_all(dev_dir.join("1-3:1.0")).unwrap();
        for (f, v) in [
            ("idVendor", "046d"),
            ("idProduct", "c31c"),
            ("product", "USB Keyboard"),
            ("authorized", "0"),
        ] {
            fs::write(dev_dir.join(f), format!("{v}\n")).unwrap();
        }
        for (f, v) in [
            ("bInterfaceClass", "03"),
            ("bInterfaceSubClass", "01"),
            ("bInterfaceProtocol", "01"),
        ] {
            fs::write(dev_dir.join("1-3:1.0").join(f), format!("{v}\n")).unwrap();
        }
        let dev = read_device(&dev_dir).unwrap();
        assert_eq!(dev.identity(), "046d:c31c:");
        assert_eq!(
            dev.interfaces,
            vec![Interface {
                class: 3,
                subclass: 1,
                protocol: 1
            }]
        );
        assert!(!dev.authorized);
        assert_eq!(decide(&Policy::default(), &dev), Verdict::Pending);
    }

    #[test]
    fn policy_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("usb/policy.json");
        assert_eq!(load_policy(&p).unwrap(), Policy::default());
        let mut pol = Policy::default();
        pol.allow.push("a:b:c".into());
        save_policy(&p, &pol).unwrap();
        assert_eq!(load_policy(&p).unwrap(), pol);
    }
}
