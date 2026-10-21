//! Window and session actions behind the shortcuts and the shell chrome:
//! close, switch, show desktop, maximize / restore / snap, and app launches.

use std::process::Command;

use smithay::{
    desktop::Window,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Point, Rectangle, SERIAL_COUNTER},
};

use crate::{config::Act, state::Smallvil};

/// Start a program detached from the compositor, in the user's home.
pub fn spawn(cmd: &[&str]) {
    spawn_env(cmd, &[]);
}

pub fn spawn_env(cmd: &[&str], env: &[(&str, &str)]) {
    let Some((prog, args)) = cmd.split_first() else { return };
    let mut c = Command::new(prog);
    c.args(args);
    c.envs(env.iter().copied());
    if let Some(home) = std::env::var_os("HOME") {
        c.current_dir(home);
    }
    if let Err(e) = c.spawn() {
        tracing::warn!(?e, prog, "spawn failed");
    }
}

impl Smallvil {
    pub fn run_act(&mut self, act: Act) {
        tracing::info!(?act, "shortcut");
        use smithay::utils::IsAlive;
        self.saved.retain(|(w, _)| w.alive());
        self.hidden.retain(|(w, _)| w.alive());
        self.minimized.retain(|(w, _)| w.alive());
        match act {
            Act::Close => {
                if let Some(t) = self.top_window().and_then(|w| w.toplevel().cloned()) {
                    t.send_close();
                }
            }
            Act::Cycle => self.open_switcher(),
            Act::Minimize => self.minimize_top(),
            Act::ShowDesktop => self.show_desktop(),
            Act::Maximize => self.place_top(Place::Max),
            Act::Restore => self.place_top(Place::Restore),
            Act::SnapLeft => self.place_top(Place::Left),
            Act::SnapRight => self.place_top(Place::Right),
            Act::Launcher => self.open_finder(),
            Act::Expose => self.open_expose(),
            Act::Lock => self.lock(),
            Act::Screenshot => self.shot = Some(None),
            Act::ScreenshotRegion => self.open_region(),
            Act::Terminal => spawn(&["sh", "-c", crate::chrome::TERMINAL]),
            Act::Files => spawn(&["sh", "-c", crate::chrome::FILES]),
            Act::Browser => spawn(&["xdg-open", "https://redrob.ai"]),
            Act::Settings => self.open_settings(),
            Act::ToggleTheme => self.chrome.toggle_theme(),
        }
    }

    /// The top-most mapped window.
    pub fn top_window(&self) -> Option<Window> {
        self.space.elements().last().cloned()
    }

    fn focus(&mut self, window: Option<Window>) {
        let serial = SERIAL_COUNTER.next_serial();
        if let Some(w) = &window {
            self.space.raise_element(w, true);
        }
        let surface = window.and_then(|w| w.toplevel().map(|t| t.wl_surface().clone()));
        let keyboard = self.seat.get_keyboard().unwrap();
        keyboard.set_focus(self, surface, serial);
    }

    pub fn minimize(&mut self, w: &Window) {
        if let Some(loc) = self.space.element_location(w) {
            self.space.unmap_elem(w);
            self.minimized.push((w.clone(), loc));
        }
        let top = self.top_window();
        self.focus(top);
    }

    fn minimize_top(&mut self) {
        if let Some(w) = self.top_window() {
            self.minimize(&w);
        }
    }

    pub fn toggle_maximize(&mut self, w: &Window) {
        self.focus(Some(w.clone()));
        self.place_top(Place::Max);
    }

    /// Hide every window; the next press brings them all back where they were.
    fn show_desktop(&mut self) {
        if self.hidden.is_empty() {
            let all: Vec<(Window, Point<i32, Logical>)> = self
                .space
                .elements()
                .filter_map(|w| Some((w.clone(), self.space.element_location(w)?)))
                .collect();
            for (w, _) in &all {
                self.space.unmap_elem(w);
            }
            self.hidden = all;
            self.focus(None);
        } else {
            for (w, loc) in std::mem::take(&mut self.hidden) {
                self.space.map_element(w, loc, false);
            }
            let top = self.top_window();
            self.focus(top);
        }
    }

    /// The part of the output windows may use (outside the bar / taskbar).
    pub fn work_area(&self) -> Rectangle<i32, Logical> {
        let out = self
            .space
            .outputs()
            .next()
            .and_then(|o| self.space.output_geometry(o))
            .unwrap_or_else(|| Rectangle::from_size((1280, 800).into()));
        let (top, bottom) = self.chrome.reserved();
        Rectangle::new(
            (out.loc.x, out.loc.y + top).into(),
            (out.size.w, (out.size.h - top - bottom).max(1)).into(),
        )
    }

    fn place_top(&mut self, place: Place) {
        let Some(window) = self.top_window() else { return };
        let Some(toplevel) = window.toplevel().cloned() else { return };
        let Some(current) = self.content_rect(&window) else { return };
        let title = if crate::deco::wants_frame(&window) { crate::theme::TITLE_H } else { 0 };
        let area = self.work_area();
        let saved = self.saved.iter().position(|(w, _)| *w == window);

        let target = match place {
            Place::Restore => match saved {
                Some(i) => Some(self.saved.remove(i).1),
                None => None,
            },
            // Maximize toggles, like the title-bar button.
            Place::Max if saved.is_some() && is_max(&toplevel) => Some(self.saved.remove(saved.unwrap()).1),
            Place::Max => Some(area),
            Place::Left => Some(Rectangle::new(area.loc, (area.size.w / 2, area.size.h).into())),
            Place::Right => Some(Rectangle::new(
                (area.loc.x + area.size.w / 2, area.loc.y).into(),
                (area.size.w - area.size.w / 2, area.size.h).into(),
            )),
        };
        let Some(mut target) = target else { return };
        // A restore target is already a content rect; the others are frames.
        let from_frame = place != Place::Restore && !(place == Place::Max && target != area);
        if from_frame && title > 0 {
            target = Rectangle::new((target.loc.x, target.loc.y + title).into(), (target.size.w, target.size.h - title).into());
        }
        if saved.is_none() && place != Place::Restore {
            self.saved.push((window.clone(), current));
        }
        let maximized = place == Place::Max && from_frame;
        toplevel.with_pending_state(|s| {
            s.size = Some(target.size);
            if maximized {
                s.states.set(xdg_toplevel::State::Maximized);
            } else {
                s.states.unset(xdg_toplevel::State::Maximized);
            }
        });
        toplevel.send_pending_configure();
        let geo_loc = window.geometry().loc;
        self.space.map_element(window.clone(), target.loc - geo_loc, true);
        self.focus(Some(window));
    }

    /// Where a new window opens: inside the work area, cascading.
    pub fn new_window_location(&mut self) -> Point<i32, Logical> {
        let area = self.work_area();
        let n = (self.space.elements().len() % 8) as i32;
        (area.loc.x + 64 + 32 * n, area.loc.y + 24 + crate::theme::TITLE_H + 28 * n).into()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Max,
    Restore,
    Left,
    Right,
}

fn is_max(t: &smithay::wayland::shell::xdg::ToplevelSurface) -> bool {
    t.with_pending_state(|s| s.states.contains(xdg_toplevel::State::Maximized))
}
