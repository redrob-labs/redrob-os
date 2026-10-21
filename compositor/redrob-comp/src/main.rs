#![allow(irrefutable_let_patterns)]

mod apps;
mod xwayland;
mod greeter;
mod display;
mod hangul;
mod settings;
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
    // `--greeter`: the login screen greetd runs (see greeter.rs).
    let greeter = std::env::args().any(|a| a == "--greeter");
    let drm = greeter || std::env::args().any(|a| a == "--drm");
    if drm {
        crate::udev::init_udev(&mut event_loop, &mut state)?;
    } else {
        unsafe { std::env::set_var("REDROB_NESTED", "1") };
        // Development: try a scale in a window (REDROB_SCALE=1.5).
        if let Some(s) = std::env::var("REDROB_SCALE").ok().and_then(|v| v.parse().ok()) {
            crate::display::set_scale(s);
        }
        crate::winit::init_winit(&mut event_loop, &mut state)?;
    }

    // Set WAYLAND_DISPLAY to our socket name, so child processes connect to Smallvil rather
    // than the host compositor
    unsafe { std::env::set_var("WAYLAND_DISPLAY", &state.socket_name) };

    // Apps started from here identify this desktop (XDG autostart, portals).
    unsafe {
        std::env::set_var("XDG_CURRENT_DESKTOP", "Redrob");
        std::env::set_var("XDG_SESSION_DESKTOP", "Redrob");
        // Qt apps ask us for server-side frames.
        std::env::set_var("QT_WAYLAND_DISABLE_WINDOWDECORATION", "1");
        // GTK apps on Wayland.
        std::env::set_var("GDK_BACKEND", "wayland,x11");
        // Korean input through Fcitx 5. Qt and GTK speak text-input-v3 to the
        // compositor, which Fcitx serves over input-method-v2, so the 한자
        // candidate window shows as an input-method popup. (Qt's own fcitx
        // module types fine but has nowhere to put that window here.) X11 apps
        // use XIM.
        std::env::set_var("QT_IM_MODULE", "wayland");
        std::env::set_var("XMODIFIERS", "@im=fcitx");
        std::env::set_var("SDL_IM_MODULE", "fcitx");
    }

    // The login session runs the desktop's autostart entries (Redrob Launcher,
    // first-run app chooser, ...); nested development runs start a test client.
    state.status = crate::status::start();
    if greeter {
        crate::greeter::start();
        state.lock();
    } else if drm {
        state.chrome.apply_app_theme();
        // Our own session bus services and status polling.
        crate::notify::serve(state.note_tx.clone());
        // D-Bus- and systemd-activated services (portals, Nautilus) need this
        // session's environment; then bring up graphical-session.target.
        // X11 apps get Xwayland; the session's services and apps start once
        // its DISPLAY is known (or it failed), so they all see it.
        state.start_xwayland(|_| {
            crate::shell::spawn(&[
                "sh",
                "-c",
                "dbus-update-activation-environment --systemd WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_DESKTOP \
                 QT_WAYLAND_DISABLE_WINDOWDECORATION QT_IM_MODULE XMODIFIERS \
                 SDL_IM_MODULE GDK_BACKEND; systemctl --user start redrob-session.target",
            ]);
            crate::shell::spawn(&["sh", "-c", "command -v fcitx5 >/dev/null && exec fcitx5 -d --replace"]);
            crate::shell::spawn(&["sh", "-c", "[ -x /usr/libexec/redrob/ptyxis-setup ] && exec /usr/libexec/redrob/ptyxis-setup"]);
            crate::autostart::run();
        });
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
