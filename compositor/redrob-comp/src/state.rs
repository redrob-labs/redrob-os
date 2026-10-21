use std::{ffi::OsString, sync::Arc};

use smithay::{
    desktop::{PopupManager, Space, Window, WindowSurfaceType},
    input::{Seat, SeatState},
    reexports::{
        calloop::{EventLoop, Interest, LoopSignal, Mode, PostAction, generic::Generic},
        wayland_server::{
            Display, DisplayHandle,
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
        },
    },
    utils::{Logical, Point},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::xdg::XdgShellState,
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

pub struct Smallvil {
    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,

    pub space: Space<Window>,
    pub loop_signal: LoopSignal,

    // Smithay State
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Smallvil>,
    pub data_device_state: DataDeviceState,
    pub popups: PopupManager,

    pub seat: Seat<Self>,

    /// Redrob bar + dock, drawn by the compositor.
    pub chrome: crate::chrome::Chrome,

    /// Windows put away by "show desktop", with where they were.
    pub hidden: Vec<(Window, Point<i32, Logical>)>,
    /// Geometry before maximize / snap, for restore.
    pub saved: Vec<(Window, smithay::utils::Rectangle<i32, Logical>)>,
    /// Key codes whose press a shortcut took, so their release is kept from
    /// the client too.
    pub eaten: Vec<u32>,
    /// A modifier went down alone: a bare "Super" tap fires on its release.
    pub tap: bool,
    /// Server-side window frames.
    pub deco: crate::deco::Decorations,
    pub xdg_decoration_state: smithay::wayland::shell::xdg::decoration::XdgDecorationState,
    /// Minimized windows, most recent last, with where they were.
    pub minimized: Vec<(Window, Point<i32, Logical>)>,
    /// Finder, switcher, expose, lock, region select, toasts.
    pub layers: crate::overlay::Layers,
    pub note_rx: std::sync::mpsc::Receiver<crate::notify::Msg>,
    pub note_tx: std::sync::mpsc::Sender<crate::notify::Msg>,
    /// A screenshot to take on the next frame: whole output or a region.
    pub shot: Option<Option<smithay::utils::Rectangle<i32, Logical>>>,
    pub status: crate::status::Shared,
    /// Input-method popups (candidate windows), placed each frame.
    pub im_popups: Vec<smithay::wayland::input_method::PopupSurface>,
    pub display: crate::display::Display,
    pub loop_handle: smithay::reexports::calloop::LoopHandle<'static, Self>,
    pub xwm: Option<smithay::xwayland::X11Wm>,
    pub xwayland_shell_state: smithay::wayland::xwayland_shell::XWaylandShellState,
}

impl Smallvil {
    pub fn new(event_loop: &mut EventLoop<'static, Self>, display: Display<Self>) -> Self {
        let start_time = std::time::Instant::now();

        let dh = display.handle();

        // Here we initialize implementations of some wayland protocols
        // Some of them require us to implement traits on the Smallvil state,
        // you can find those implementations in the `crate::handlers` module

        // Initialize protocols needed for displaying windows
        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let popups = PopupManager::default();

        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);

        // Data device is responsible for clipboard and drag-and-drop
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        // Server-side decorations: Redrob draws the window frames.
        // Text input + input method + virtual keyboard: Fcitx 5 (Korean) talks
        // to apps through these.
        smithay::wayland::text_input::TextInputManagerState::new::<Self>(&dh);
        smithay::wayland::input_method::InputMethodManagerState::new::<Self, _>(&dh, |_client| true);
        smithay::wayland::virtual_keyboard::VirtualKeyboardManagerState::new::<Self, _>(&dh, |_client| true);
        let xdg_decoration_state =
            smithay::wayland::shell::xdg::decoration::XdgDecorationState::new::<Self>(&dh);

        // A seat is a group of keyboards, pointer and touch devices.
        // A seat typically has a pointer and maintains a keyboard focus and a pointer focus.
        let mut seat_state = SeatState::new();
        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "winit");

        // Notify clients that we have a keyboard, for the sake of the example we assume that keyboard is always present.
        // You may want to track keyboard hot-plug in real compositor.
        // Korean keyboards: right Alt is 한/영 and right Ctrl is 한자 (the usual
        // layout of laptops without dedicated keys); Fcitx 5 toggles on Hangul.
        let xkb = smithay::input::keyboard::XkbConfig {
            options: Some("korean:ralt_hangul,korean:rctrl_hanja".into()),
            ..Default::default()
        };
        seat.add_keyboard(xkb, 200, 25).unwrap();

        // Notify clients that we have a pointer (mouse)
        // Here we assume that there is always pointer plugged in
        seat.add_pointer();

        // A space represents a two-dimensional plane. Windows and Outputs can be mapped onto it.
        //
        // Windows get a position and stacking order through mapping.
        // Outputs become views of a part of the Space and can be rendered via Space::render_output.
        let space = Space::default();
        let (note_tx, note_rx) = std::sync::mpsc::channel();

        // Setup a wayland socket that will be used to accept clients
        let socket_name = Self::init_wayland_listener(display, event_loop);

        // Get the loop signal, used to stop the event loop
        let loop_signal = event_loop.get_signal();
        let xwayland_shell_state = smithay::wayland::xwayland_shell::XWaylandShellState::new::<Self>(&dh);

        Self {
            start_time,
            display_handle: dh,

            space,
            loop_signal,
            socket_name,

            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            popups,
            seat,
            chrome: crate::chrome::Chrome::new(),
            hidden: Vec::new(),
            saved: Vec::new(),
            eaten: Vec::new(),
            tap: false,
            deco: Default::default(),
            xdg_decoration_state,
            minimized: Vec::new(),
            layers: Default::default(),
            note_rx,
            note_tx,
            shot: None,
            status: Default::default(),
            im_popups: Vec::new(),
            display: Default::default(),
            loop_handle: event_loop.handle(),
            xwm: None,
            xwayland_shell_state,
        }
    }

    fn init_wayland_listener(display: Display<Smallvil>, event_loop: &mut EventLoop<Self>) -> OsString {
        // Creates a new listening socket, automatically choosing the next available `wayland` socket name.
        let listening_socket = ListeningSocketSource::new_auto().unwrap();

        // Get the name of the listening socket.
        // Clients will connect to this socket.
        let socket_name = listening_socket.socket_name().to_os_string();

        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                // Inside the callback, you should insert the client into the display.
                //
                // You may also associate some data with the client when inserting the client.
                state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                    .unwrap();
            })
            .expect("Failed to init the wayland event source.");

        // You also need to add the display itself to the event loop, so that client events will be processed by wayland-server.
        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // Safety: we don't drop the display
                    unsafe {
                        display.get_mut().dispatch_clients(state).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();

        socket_name
    }

    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.space.element_under(pos).and_then(|(window, location)| {
            window
                .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, p)| (s, (p + location).to_f64()))
        })
    }
}

/// Data associated with a wayland client that connects to Smallvil.
/// One instance of this type per client.
#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
