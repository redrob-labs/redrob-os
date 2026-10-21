use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
        KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent,
    },
    input::{
        keyboard::{FilterResult, KeysymHandle, ModifiersState, xkb},
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::SERIAL_COUNTER,
};

use crate::state::Smallvil;

impl Smallvil {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time(&event);

                let code = event.key_code();
                let pressed = event.state() == KeyState::Pressed;
                let act = self.seat.get_keyboard().unwrap().input::<Option<crate::config::Act>, _>(
                    self,
                    code,
                    event.state(),
                    serial,
                    time,
                    |state, mods, key| state.match_shortcut(code.raw(), pressed, mods, &key),
                );
                if let Some(Some(act)) = act {
                    self.run_act(act);
                }
            }
            InputEvent::PointerMotion { event, .. } => {
                // Relative mouse (real hardware): move and clamp to the output.
                use smithay::backend::input::PointerMotionEvent;
                let pointer = self.seat.get_pointer().unwrap();
                let mut pos = pointer.current_location() + event.delta();
                // The screen under the new x (screens sit side by side).
                let geos: Vec<_> = self.space.outputs().filter_map(|o| self.space.output_geometry(o)).map(|g| g.to_f64()).collect();
                let geo = geos
                    .iter()
                    .find(|g| pos.x >= g.loc.x && pos.x < g.loc.x + g.size.w)
                    .or_else(|| if pos.x < 0.0 { geos.first() } else { geos.iter().max_by(|a, b| a.loc.x.total_cmp(&b.loc.x)) })
                    .copied()
                    .unwrap();
                pos.x = pos.x.clamp(geo.loc.x, geo.loc.x + geo.size.w - 1.0);
                pos.y = pos.y.clamp(geo.loc.y, geo.loc.y + geo.size.h - 1.0);
                let under = self.surface_under(pos);
                let serial = SERIAL_COUNTER.next_serial();
                pointer.motion(self, under, &MotionEvent { location: pos, serial, time: event.time() });
                pointer.frame(self);
                self.track_hover(pos);
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let output = self.space.outputs().next().unwrap();

                let output_geo = self.space.output_geometry(output).unwrap();

                let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
                tracing::debug!(?pos, "abs motion");

                let serial = SERIAL_COUNTER.next_serial();

                let pointer = self.seat.get_pointer().unwrap();

                let under = self.surface_under(pos);

                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location: pos,
                        serial,
                        time: event.time(),
                    },
                );
                pointer.frame(self);
                self.track_hover(pos);
            }
            InputEvent::PointerButton { event, .. } => {
                let pointer = self.seat.get_pointer().unwrap();
                let keyboard = self.seat.get_keyboard().unwrap();

                let serial = SERIAL_COUNTER.next_serial();

                let button = event.button_code();

                let button_state = event.state();

                // Overlays (finder, switcher, expose, lock, region, toasts) first.
                if self.layers.overlay.is_some() || !self.layers.toasts.is_empty() {
                    let at = pointer.current_location();
                    if self.overlay_button(button_state == ButtonState::Pressed, at) {
                        return;
                    }
                }
                // The Redrob bar/dock sit above every window: a press on them is ours.
                if ButtonState::Pressed == button_state {
                    let hit = self.chrome.hit(pointer.current_location());
                    tracing::info!(loc = ?pointer.current_location(), ?hit, button, "press");
                    if let Some(action) = hit {
                        if action == crate::chrome::Action::Start {
                            self.open_finder();
                        } else {
                            self.chrome.act(action);
                            if std::mem::take(&mut self.chrome.want_settings) {
                                self.open_settings();
                            }
                        }
                        return;
                    }
                    if button == 0x110 {
                        if let Some((window, part)) = self.deco_hit(pointer.current_location()) {
                            self.frame_press(window, part, serial);
                            return;
                        }
                    }
                }

                if ButtonState::Pressed == button_state && !pointer.is_grabbed() {
                    if let Some((window, _loc)) = self
                        .space
                        .element_under(pointer.current_location())
                        .map(|(w, l)| (w.clone(), l))
                    {
                        self.space.raise_element(&window, true);
                        keyboard.set_focus(
                            self,
                            Some(window.toplevel().unwrap().wl_surface().clone()),
                            serial,
                        );
                        self.space.elements().for_each(|window| {
                            window.toplevel().unwrap().send_pending_configure();
                        });
                    } else {
                        self.space.elements().for_each(|window| {
                            window.set_activated(false);
                            window.toplevel().unwrap().send_pending_configure();
                        });
                        keyboard.set_focus(self, Option::<WlSurface>::None, serial);
                    }
                };

                pointer.button(
                    self,
                    &ButtonEvent {
                        button,
                        state: button_state,
                        serial,
                        time: event.time(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerAxis { event, .. } => {
                let source = event.source();

                let horizontal_amount = event
                    .amount(Axis::Horizontal)
                    .unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.);
                let vertical_amount = event
                    .amount(Axis::Vertical)
                    .unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.);
                let horizontal_amount_discrete = event.amount_v120(Axis::Horizontal);
                let vertical_amount_discrete = event.amount_v120(Axis::Vertical);

                let mut frame = AxisFrame::new(event.time()).source(source);
                if horizontal_amount != 0.0 {
                    frame = frame.value(Axis::Horizontal, horizontal_amount);
                    if let Some(discrete) = horizontal_amount_discrete {
                        frame = frame.v120(Axis::Horizontal, discrete as i32);
                    }
                }
                if vertical_amount != 0.0 {
                    frame = frame.value(Axis::Vertical, vertical_amount);
                    if let Some(discrete) = vertical_amount_discrete {
                        frame = frame.v120(Axis::Vertical, discrete as i32);
                    }
                }

                if source == AxisSource::Finger {
                    if event.amount(Axis::Horizontal) == Some(0.0) {
                        frame = frame.stop(Axis::Horizontal);
                    }
                    if event.amount(Axis::Vertical) == Some(0.0) {
                        frame = frame.stop(Axis::Vertical);
                    }
                }

                let pointer = self.seat.get_pointer().unwrap();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            InputEvent::DeviceAdded { device } => tracing::info!(name = smithay::backend::input::Device::name(&device), "input device"),
            _ => {}
        }
    }
}

impl Smallvil {
    /// Shortcut matching for one key event. A press that matches a binding is
    /// taken from the client (and so is its release); everything else passes
    /// through. A bare-modifier binding (`Super` on the Windows layout) fires
    /// when that key is released without any other key in between.
    fn match_shortcut(
        &mut self,
        code: u32,
        pressed: bool,
        mods: &ModifiersState,
        key: &KeysymHandle<'_>,
    ) -> FilterResult<Option<crate::config::Act>> {
        use crate::config::{ALT, CTRL, SHIFT, SUPER};
        let name = key
            .raw_syms()
            .first()
            .map(|s| xkb::keysym_get_name(*s).to_ascii_lowercase())
            .unwrap_or_default();
        let is_super = name == "super_l" || name == "super_r";

        // An overlay takes the keyboard (a release a shortcut already took
        // stays eaten). The switcher still needs Tab with the modifier held.
        if self.layers.overlay.is_some() {
            if !pressed {
                if let Some(i) = self.eaten.iter().position(|c| *c == code) {
                    self.eaten.remove(i);
                    if !matches!(self.layers.overlay, Some(crate::overlay::Overlay::Switcher { .. })) {
                        return FilterResult::Intercept(None);
                    }
                }
            }
            self.tap = false;
            self.overlay_key(pressed, mods, key);
            return FilterResult::Intercept(None);
        }

        if !pressed {
            if let Some(i) = self.eaten.iter().position(|c| *c == code) {
                self.eaten.remove(i);
                return FilterResult::Intercept(None);
            }
            if is_super && self.tap {
                self.tap = false;
                let bare = self.chrome.shortcuts().iter().find(|(c, _)| c.mods == SUPER && c.key.is_empty());
                if let Some((_, act)) = bare {
                    return FilterResult::Intercept(Some(*act));
                }
            }
            return FilterResult::Forward;
        }

        if is_super {
            self.tap = !mods.ctrl && !mods.alt && !mods.shift;
            return FilterResult::Forward;
        }
        self.tap = false;
        let mut bits = 0;
        if mods.logo {
            bits |= SUPER;
        }
        if mods.ctrl {
            bits |= CTRL;
        }
        if mods.alt {
            bits |= ALT;
        }
        if mods.shift {
            bits |= SHIFT;
        }
        let hit = self.chrome.shortcuts().iter().find(|(c, _)| c.mods == bits && c.key == name).map(|(_, a)| *a);
        match hit {
            Some(act) => {
                self.eaten.push(code);
                FilterResult::Intercept(Some(act))
            }
            None => FilterResult::Forward,
        }
    }
}

impl Smallvil {
    fn track_hover(&mut self, pos: smithay::utils::Point<f64, smithay::utils::Logical>) {
        if self.layers.overlay.is_some() {
            self.overlay_motion(pos);
        }
        self.chrome.pointer(pos);
        self.deco.hover = self.deco_hit(pos);
    }

    /// A left press on a window frame: the dots / buttons act, the title bar
    /// raises the window and starts a move.
    fn frame_press(&mut self, window: smithay::desktop::Window, part: crate::deco::DecoHit, serial: smithay::utils::Serial) {
        use crate::deco::DecoHit;
        let keyboard = self.seat.get_keyboard().unwrap();
        self.space.raise_element(&window, true);
        if let Some(t) = window.toplevel() {
            keyboard.set_focus(self, Some(t.wl_surface().clone()), serial);
        }
        match part {
            DecoHit::Close => {
                if let Some(t) = window.toplevel() {
                    t.send_close();
                }
            }
            DecoHit::Minimize => self.minimize(&window),
            DecoHit::Maximize => self.toggle_maximize(&window),
            DecoHit::Title => {
                let pointer = self.seat.get_pointer().unwrap();
                let Some(initial_window_location) = self.space.element_location(&window) else { return };
                let start_data = smithay::input::pointer::GrabStartData {
                    focus: None,
                    button: 0x110,
                    location: pointer.current_location(),
                };
                let grab = crate::grabs::MoveSurfaceGrab { start_data, window, initial_window_location };
                pointer.set_grab(self, grab, serial, smithay::input::pointer::Focus::Clear);
            }
        }
    }
}

impl smithay::wayland::text_input::TextInputActivation for Smallvil {}

/// Keys the input method sends back (Fcitx passes through what it does not
/// compose): straight to the focused client, no shell shortcuts.
impl smithay::wayland::virtual_keyboard::VirtualKeyboardHandler for Smallvil {
    fn process_virtual_keyboard_event(
        &mut self,
        event: InputEvent<smithay::wayland::virtual_keyboard::VirtualKeyboardBackend>,
    ) {
        use smithay::wayland::virtual_keyboard::VirtualKeyboardSpecialEvent as Sp;
        let keyboard = self.seat.get_keyboard().unwrap();
        match event {
            InputEvent::DeviceAdded { device } | InputEvent::Special(Sp::KeymapChanged { device }) => {
                if let Some(keymap) = device.keymap() {
                    if let Err(e) = keyboard.set_keymap_from_string(self, keymap.to_string()) {
                        tracing::warn!(?e, "virtual keyboard keymap");
                    }
                }
            }
            InputEvent::Keyboard { event } => {
                use smithay::wayland::input_method::InputMethodSeat;
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time(&event);
                // Keys the input method passes through must not go back into
                // its own grab (that loops); they go straight to the client.
                let grab = self.seat.input_method().keyboard_grab_client();
                if grab.is_some() && grab == Event::device(&event).client() {
                    keyboard.input_forward_bypassing_grab(self, event.key_code(), event.state(), serial, time, true);
                } else {
                    keyboard.input::<(), _>(self, event.key_code(), event.state(), serial, time, |_, _, _| FilterResult::Forward);
                }
            }
            InputEvent::Special(Sp::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. }) => {
                keyboard.with_xkb_state(self, |mut c| c.set_modifier_mask(mods_depressed, mods_latched, mods_locked, group));
            }
            _ => {}
        }
    }
}
