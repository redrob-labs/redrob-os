//! X11 apps through Xwayland (Steam and the game launchers, Chromium-based
//! apps started without Wayland flags). redrob-comp is their window
//! manager: they map into the same space as Wayland windows and draw their
//! own frames; the clipboard is shared both ways.

use smithay::wayland::seat::WaylandFocus;
use std::os::unix::io::OwnedFd;

use smithay::{
    desktop::Window,
    utils::{Logical, Rectangle},
    wayland::{
        selection::{
            SelectionTarget,
            data_device::{
                clear_data_device_selection, current_data_device_selection_userdata,
                request_data_device_client_selection, set_data_device_selection,
            },
        },
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
        xwm::{Reorder, ResizeEdge, XwmId},
    },
};

use crate::{Smallvil, handlers::Selection};

impl XWaylandShellHandler for Smallvil {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }
}

impl Smallvil {
    fn x11_window(&self, s: &X11Surface) -> Option<Window> {
        self.space.elements().find(|w| w.x11_surface() == Some(s)).cloned()
    }

    /// Start Xwayland; `ready` runs once its DISPLAY is set (or it failed).
    pub fn start_xwayland(&mut self, ready: impl FnOnce(&mut Smallvil) + 'static) {
        let spawned = XWayland::spawn(
            &self.display_handle,
            None,
            std::iter::empty::<(String, String)>(),
            std::iter::empty::<String>(),
            true,
            std::process::Stdio::null(),
            std::process::Stdio::null(),
            |_| (),
        );
        let (xwayland, client) = match spawned {
            Ok(x) => x,
            Err(e) => {
                tracing::warn!(?e, "xwayland: not started");
                ready(self);
                return;
            }
        };
        let dh = self.display_handle.clone();
        let handle = self.loop_handle.clone();
        let mut ready = Some(ready);
        let r = self.loop_handle.insert_source(xwayland, move |event, _, state| match event {
            XWaylandEvent::Ready { x11_socket, display_number } => {
                match X11Wm::start_wm(handle.clone(), &dh, x11_socket, client.clone()) {
                    Ok(wm) => state.xwm = Some(wm),
                    Err(e) => tracing::warn!(?e, "xwayland: window manager"),
                }
                unsafe { std::env::set_var("DISPLAY", format!(":{display_number}")) };
                tracing::info!(display_number, "xwayland: ready");
                if let Some(f) = ready.take() {
                    f(state);
                }
            }
            XWaylandEvent::Error => {
                tracing::warn!("xwayland: failed to start");
                if let Some(f) = ready.take() {
                    f(state);
                }
            }
        });
        if let Err(e) = r {
            tracing::warn!(?e, "xwayland: event source");
        }
    }
}

impl XwmHandler for Smallvil {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm.as_mut().unwrap()
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        let _ = surface.set_mapped(true);
        let at = self.new_window_location();
        let mut geo = surface.geometry();
        geo.loc = at;
        let _ = surface.configure(geo);
        let window = Window::new_x11_window(surface);
        self.space.map_element(window.clone(), at, true);
        self.focus(Some(window));
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        // Menus and tooltips place themselves.
        let at = surface.geometry().loc;
        self.space.map_element(Window::new_x11_window(surface), at, true);
    }

    fn unmapped_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        if let Some(w) = self.x11_window(&surface) {
            self.space.unmap_elem(&w);
            let top = self.top_window();
            self.focus(top);
        }
        if !surface.is_override_redirect() {
            let _ = surface.set_mapped(false);
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        surface: X11Surface,
        _x: Option<i32>,
        _y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        // A new size is fine; where the window sits is ours to decide.
        let mut geo = surface.geometry();
        if let Some(w) = w {
            geo.size.w = w as i32;
        }
        if let Some(h) = h {
            geo.size.h = h as i32;
        }
        let _ = surface.configure(geo);
    }

    fn configure_notify(&mut self, _xwm: XwmId, surface: X11Surface, geometry: Rectangle<i32, Logical>, _above: Option<u32>) {
        if let Some(w) = self.x11_window(&surface) {
            self.space.map_element(w, geometry.loc, false);
        }
    }

    fn maximize_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        let area = self.work_area();
        let _ = surface.set_maximized(true);
        let _ = surface.configure(area);
        if let Some(w) = self.x11_window(&surface) {
            self.space.map_element(w, area.loc, true);
        }
    }

    fn unmaximize_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        let _ = surface.set_maximized(false);
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        // Games: the whole main screen, above the bar.
        let Some(out) = self.space.outputs().next().and_then(|o| self.space.output_geometry(o)) else { return };
        let _ = surface.set_fullscreen(true);
        let _ = surface.configure(out);
        if let Some(w) = self.x11_window(&surface) {
            self.space.map_element(w, out.loc, true);
        }
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        let _ = surface.set_fullscreen(false);
        let area = self.work_area();
        let mut geo = surface.geometry();
        geo.loc = area.loc;
        let _ = surface.configure(geo);
        if let Some(w) = self.x11_window(&surface) {
            self.space.map_element(w, area.loc, true);
        }
    }

    fn resize_request(&mut self, _xwm: XwmId, _surface: X11Surface, _button: u32, _edges: ResizeEdge) {}

    fn move_request(&mut self, _xwm: XwmId, surface: X11Surface, _button: u32) {
        let Some(w) = self.x11_window(&surface) else { return };
        let pointer = self.seat.get_pointer().unwrap();
        let Some(start_data) = pointer.grab_start_data() else { return };
        let Some(initial) = self.space.element_location(&w) else { return };
        let grab = crate::grabs::MoveSurfaceGrab { start_data, window: w, initial_window_location: initial };
        pointer.set_grab(self, grab, smithay::utils::SERIAL_COUNTER.next_serial(), smithay::input::pointer::Focus::Clear);
    }

    fn allow_selection_access(&mut self, xwm: XwmId, _selection: SelectionTarget) -> bool {
        // Only the X11 window that has the keyboard reads the clipboard.
        let focus = self.seat.get_keyboard().and_then(|k| k.current_focus());
        self.space
            .elements()
            .filter_map(|w| w.x11_surface().map(|x| (x.clone(), w.wl_surface().map(|s| s.into_owned()))))
            .any(|(x, s)| x.xwm_id() == Some(xwm) && s.is_some() && s == focus)
    }

    fn send_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_type: String, fd: OwnedFd) {
        if selection == SelectionTarget::Clipboard {
            if let Err(e) = request_data_device_client_selection(&self.seat, mime_type, fd) {
                tracing::debug!(?e, "xwayland: clipboard for X11");
            }
        }
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        if selection == SelectionTarget::Clipboard {
            set_data_device_selection(&self.display_handle, &self.seat, mime_types, Selection::X11);
        }
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        if selection == SelectionTarget::Clipboard && current_data_device_selection_userdata(&self.seat).is_some() {
            clear_data_device_selection(&self.display_handle, &self.seat);
        }
    }

    fn disconnected(&mut self, _xwm: XwmId) {
        self.xwm = None;
    }
}
