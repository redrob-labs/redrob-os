mod compositor;
mod xdg_shell;

use crate::Smallvil;

//
// Wl Seat
//

use smithay::input::dnd::{DnDGrab, DndGrabHandler, GrabType, Source};
use smithay::input::pointer::Focus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Serial;
use smithay::wayland::output::OutputHandler;
use smithay::wayland::pointer_constraints::PointerConstraintsHandler;
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    DataDeviceHandler, DataDeviceState, WaylandDndGrabHandler, set_data_device_focus,
};

impl SeatHandler for Smallvil {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Smallvil> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: smithay::input::pointer::CursorImageStatus) {}

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client);
    }
}

impl PointerConstraintsHandler for Smallvil {}

//
// Wl Data Device
//

impl SelectionHandler for Smallvil {
    type SelectionUserData = ();
}

impl DataDeviceHandler for Smallvil {
    fn data_device_state(&mut self) -> &mut DataDeviceState {
        &mut self.data_device_state
    }
}

impl DndGrabHandler for Smallvil {}
impl WaylandDndGrabHandler for Smallvil {
    fn dnd_requested<S: Source>(
        &mut self,
        source: S,
        _icon: Option<WlSurface>,
        seat: Seat<Self>,
        serial: Serial,
        type_: GrabType,
    ) {
        match type_ {
            GrabType::Pointer => {
                let ptr = seat.get_pointer().unwrap();
                let start_data = ptr.grab_start_data().unwrap();

                // create a dnd grab to start the operation
                let grab = DnDGrab::new_pointer(&self.display_handle, start_data, source, seat);
                ptr.set_grab(self, grab, serial, Focus::Keep);
            }
            GrabType::Touch => {
                // smallvil lacks touch handling
                source.cancel();
            }
        }
    }
}

//
// Wl Output & Xdg Output
//

impl OutputHandler for Smallvil {}

impl smithay::wayland::input_method::InputMethodHandler for Smallvil {
    fn new_popup(&mut self, surface: smithay::wayland::input_method::PopupSurface) {
        self.im_popups.push(surface.clone());
        if let Err(e) = self.popups.track_popup(smithay::desktop::PopupKind::from(surface)) {
            tracing::warn!(?e, "input method popup");
        }
    }
    fn popup_repositioned(&mut self, _: smithay::wayland::input_method::PopupSurface) {}
    fn dismiss_popup(&mut self, surface: smithay::wayland::input_method::PopupSurface) {
        self.im_popups.retain(|p| p != &surface);
        if let Some(parent) = surface.get_parent().map(|p| p.surface.clone()) {
            let _ = smithay::desktop::PopupManager::dismiss_popup(&parent, &smithay::desktop::PopupKind::from(surface));
        }
    }
    fn parent_geometry(&self, parent: &WlSurface) -> smithay::utils::Rectangle<i32, smithay::utils::Logical> {
        self.space
            .elements()
            .find_map(|w| (w.toplevel().map(|t| t.wl_surface()) == Some(parent)).then(|| w.geometry()))
            .unwrap_or_default()
    }
}

smithay::delegate_dispatch2!(Smallvil);

impl Smallvil {
    /// Put input-method popups (the 한자 candidate window) under the text
    /// cursor instead of over it, and keep them on screen: above the cursor
    /// when there is no room below, shifted left at the right edge.
    pub fn place_im_popups(&mut self) {
        use smithay::utils::Point;
        self.im_popups.retain(|p| p.alive());
        let out = self.out_size();
        for popup in &self.im_popups {
            let Some(parent) = popup.get_parent().map(|p| p.surface.clone()) else { continue };
            let Some(win) = self.space.elements().find(|w| w.toplevel().map(|t| t.wl_surface()) == Some(&parent)).cloned() else { continue };
            let Some(wloc) = self.space.element_location(&win) else { continue };
            let size = smithay::desktop::utils::bbox_from_surface_tree(popup.wl_surface(), (0, 0)).size;
            let r = popup.text_input_rectangle();
            let gap = 4;
            let mut loc: Point<i32, smithay::utils::Logical> = (r.loc.x, r.loc.y + r.size.h.max(1) + gap).into();
            let abs_bottom = wloc.y + loc.y + size.h;
            if abs_bottom > out.h {
                loc.y = r.loc.y - size.h - gap;
            }
            let abs_right = wloc.x + loc.x + size.w;
            if abs_right > out.w {
                loc.x -= abs_right - out.w;
            }
            if wloc.x + loc.x < 0 {
                loc.x = -wloc.x;
            }
            if popup.location() != loc {
                popup.set_location(loc);
            }
        }
    }
}
