//! `udevadm monitor --udev --property` output -> events. We shell out to udevadm
//! (always present, systemd-udevd is the device manager) instead of linking libudev.
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub action: String,
    pub subsystem: String,
    pub devpath: String,
    pub props: BTreeMap<String, String>,
}

impl Event {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.props.get(k).map(String::as_str)
    }
    pub fn devtype(&self) -> Option<&str> {
        self.get("DEVTYPE")
    }
}

/// Feed lines; a blank line ends one event block. Returns the parsed event when a
/// block completes. Header lines (`UDEV  [123.4] add  /devices/... (usb)`) are
/// skipped: the properties carry ACTION, SUBSYSTEM and DEVPATH anyway.
#[derive(Default)]
pub struct Parser {
    props: BTreeMap<String, String>,
}

impl Parser {
    pub fn feed(&mut self, line: &str) -> Option<Event> {
        let line = line.trim_end();
        if line.is_empty() {
            if self.props.is_empty() {
                return None;
            }
            let props = std::mem::take(&mut self.props);
            let action = props.get("ACTION").cloned().unwrap_or_default();
            let subsystem = props.get("SUBSYSTEM").cloned().unwrap_or_default();
            let devpath = props.get("DEVPATH").cloned().unwrap_or_default();
            if action.is_empty() || devpath.is_empty() {
                return None;
            }
            return Some(Event {
                action,
                subsystem,
                devpath,
                props,
            });
        }
        if line.starts_with("UDEV")
            || line.starts_with("KERNEL")
            || line.starts_with("monitor will")
            || line.starts_with("UDEV - ")
            || line.starts_with("KERNEL - ")
        {
            return None;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            {
                self.props.insert(k.to_string(), v.to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usb_add_block() {
        let text = "monitor will print the received events for:\nUDEV - the event which udev sends out after rule processing\n\nUDEV  [12.3] add      /devices/pci0000:00/0000:00:1d.0/usb1/1-2 (usb)\nACTION=add\nDEVPATH=/devices/pci0000:00/0000:00:1d.0/usb1/1-2\nSUBSYSTEM=usb\nDEVTYPE=usb_device\nID_VENDOR_ID=0781\nPRODUCT=781/5567/126\n\nUDEV  [12.4] add      /devices/pci0000:00/0000:00:1d.0/usb1/1-2/1-2:1.0 (usb)\nACTION=add\nDEVPATH=/devices/pci0000:00/0000:00:1d.0/usb1/1-2/1-2:1.0\nSUBSYSTEM=usb\nDEVTYPE=usb_interface\n\n";
        let mut p = Parser::default();
        let evs: Vec<Event> = text.lines().filter_map(|l| p.feed(l)).collect();
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0].action, "add");
        assert_eq!(evs[0].devtype(), Some("usb_device"));
        assert_eq!(evs[0].devpath, "/devices/pci0000:00/0000:00:1d.0/usb1/1-2");
        assert_eq!(evs[0].get("ID_VENDOR_ID"), Some("0781"));
        assert_eq!(evs[1].devtype(), Some("usb_interface"));
    }

    #[test]
    fn block_without_action_is_dropped() {
        let mut p = Parser::default();
        assert!(p.feed("FOO=bar").is_none());
        assert!(p.feed("").is_none());
        assert!(p.feed("").is_none());
    }
}
