#![allow(irrefutable_let_patterns)]

mod apps;
mod chrome;
mod notify;
mod overlay;
mod pam;
mod shot;
mod status;
mod deco;
mod draw;
mod scene;
mod config;
mod autostart;
mod shell;
mod handlers;

mod grabs;
mod input;
mod state;
mod theme;
mod udev;
mod winit;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
pub use state::Smallvil;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let mut event_loop: EventLoop<Smallvil> = EventLoop::try_new()?;

    let display: Display<Smallvil> = Display::new()?;

    let mut state = Smallvil::new(&mut event_loop, display);

    // `--drm` (the login session): own the GPU and input devices directly.
    // Otherwise run nested in a window of the current desktop (development).
    let drm = std::env::args().any(|a| a == "--drm");
    if drm {
        crate::udev::init_udev(&mut event_loop, &mut state)?;
    } else {
        unsafe { std::env::set_var("REDROB_NESTED", "1") };
        crate::winit::init_winit(&mut event_loop, &mut state)?;
    }

    // Set WAYLAND_DISPLAY to our socket name, so child processes connect to Smallvil rather
    // than the host compositor
    unsafe { std::env::set_var("WAYLAND_DISPLAY", &state.socket_name) };

    // Apps started from here identify this desktop (XDG autostart, portals).
    unsafe {
        std::env::set_var("XDG_CURRENT_DESKTOP", "Redrob");
        std::env::set_var("XDG_SESSION_DESKTOP", "Redrob");
        // Qt/KDE apps: the KDE platform theme reads kdeglobals (Breeze widgets,
        // the Redrob colour scheme, fonts, icons). Without it Qt falls back to
        // its bare built-in palette. They also ask us for server-side frames.
        if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
            std::env::set_var("QT_QPA_PLATFORMTHEME", "kde");
        }
        std::env::set_var("QT_WAYLAND_DISABLE_WINDOWDECORATION", "1");
        // Breeze draws with the colour scheme, so a theme switch repaints open
        // apps. (A Kvantum theme picked for the Plasma session paints its own
        // fixed colours; the Redrob session overrides it, Plasma keeps it.)
        if std::env::var_os("QT_STYLE_OVERRIDE").is_none() {
            std::env::set_var("QT_STYLE_OVERRIDE", "Breeze");
        }
        // GTK apps on Wayland.
        std::env::set_var("GDK_BACKEND", "wayland,x11");
    }

    // The login session runs the desktop's autostart entries (Redrob Launcher,
    // first-run app chooser, ...); nested development runs start a test client.
    state.status = crate::status::start();
    if drm {
        state.chrome.apply_app_theme();
        // Our own session bus services and status polling.
        crate::notify::serve(state.note_tx.clone());
        crate::autostart::run();
    } else {
        spawn_client();
    }

    event_loop.run(None, &mut state, move |_| {
        // Smallvil is running
    })?;

    Ok(())
}

fn init_logging() {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_writer(std::io::stderr).with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    }
}

fn spawn_client() {
    let mut args = std::env::args().skip(1);
    let flag = args.next();
    let arg = args.next();

    match (flag.as_deref(), arg) {
        (Some("-c") | Some("--command"), Some(command)) => {
            std::process::Command::new(command).spawn().ok();
        }
        _ => {
            std::process::Command::new("xfce4-terminal").arg("--disable-server").spawn().ok();
        }
    }
}
