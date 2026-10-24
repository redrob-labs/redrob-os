//! Desktop notifications: an org.freedesktop.Notifications server on the
//! session bus (apps call Notify), shown as toasts by the shell.

use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
        mpsc::Sender,
    },
};

use zbus::zvariant::OwnedValue;

#[derive(Clone, Debug)]
pub struct Note {
    pub id: u32,
    pub app: String,
    pub icon: String,
    pub summary: String,
    pub body: String,
    /// Milliseconds; <= 0 means the shell default.
    pub timeout: i32,
    /// The `image-path` hint: a PNG drawn full size under the text (the
    /// pairing QR code).
    pub image: String,
    /// `urgency` 2 (critical): stays until replaced, closed or clicked.
    pub sticky: bool,
}

pub enum Msg {
    Show(Note),
    Close(u32),
}

struct Server {
    tx: Mutex<Sender<Msg>>,
    next: AtomicU32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id != 0 { replaces_id } else { self.next.fetch_add(1, Ordering::Relaxed) };
        let image = ["image-path", "image_path"]
            .iter()
            .find_map(|k| hints.get(*k).and_then(|v| <&str>::try_from(v).ok()))
            .map(|s| s.strip_prefix("file://").unwrap_or(s).to_string())
            .unwrap_or_default();
        let sticky = hints.get("urgency").and_then(|v| u8::try_from(v).ok()) == Some(2);
        let note =
            Note { id, app: app_name, icon: app_icon, summary, body: strip_markup(&body), timeout: expire_timeout, image, sticky };
        let _ = self.tx.lock().map(|t| t.send(Msg::Show(note)));
        id
    }

    fn close_notification(&self, id: u32) {
        let _ = self.tx.lock().map(|t| t.send(Msg::Close(id)));
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "body-images".into(), "icon-static".into()]
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        ("Redrob".into(), "Redrob".into(), env!("CARGO_PKG_VERSION").into(), "1.2".into())
    }
}

/// Toasts show plain text: drop the small HTML subset the spec allows.
fn strip_markup(s: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            c if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"")
}

/// Serve on the session bus from a background thread. Messages arrive on `tx`.
pub fn serve(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let server = Server { tx: Mutex::new(tx), next: AtomicU32::new(1) };
        let conn = zbus::blocking::connection::Builder::session()
            .and_then(|b| b.serve_at("/org/freedesktop/Notifications", server))
            .and_then(|b| b.name("org.freedesktop.Notifications"))
            .and_then(|b| b.build());
        match conn {
            Ok(c) => {
                tracing::info!("notifications: serving org.freedesktop.Notifications");
                // Keep the connection (and its executor thread) alive.
                loop {
                    std::thread::park();
                    let _ = &c;
                }
            }
            Err(e) => tracing::warn!(?e, "notifications: could not take the bus name"),
        }
    });
}
