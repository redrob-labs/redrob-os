//! Real-hardware backend: libseat session, udev GPU, DRM/KMS scan-out through
//! Smithay's DrmCompositor, libinput for keyboard/mouse/tablet. One GPU, every
//! connected screen side by side (the first is the main screen: it carries
//! the bar, the dock and the overlays). Screens plugged in or out while the
//! session runs are picked up from udev. Software EGL (llvmpipe on a
//! KMS-only device such as QEMU's bochs) is accepted on purpose.
//! Structure follows Smithay's `anvil` udev backend (MIT).

use std::{cell::RefCell, rc::Rc, time::Duration};

use smithay::{
    backend::{
        allocator::{
            Fourcc,
            format::FormatSet,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{
            DrmDevice, DrmDeviceFd, DrmEvent,
            compositor::{DrmCompositor, FrameFlags},
            exporter::gbm::{GbmFramebufferExporter, NodeFilter},
        },
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{ImportDma, gles::GlesRenderer},
        session::{Event as SessionEvent, Session, libseat::LibSeatSession},
        udev::{UdevBackend, UdevEvent, all_gpus, primary_gpu},
    },
    output::{Mode as WlMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{
            EventLoop,
            timer::{TimeoutAction, Timer},
        },
        drm::control::{Device as ControlDevice, Mode, ModeTypeFlags, connector, crtc},
        input::Libinput,
        rustix::fs::OFlags,
    },
    utils::{DeviceFd, Logical, Point, Rectangle, Transform},
};

use crate::Smallvil;

type Compositor =
    DrmCompositor<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

/// One screen.
struct Head {
    conn: connector::Handle,
    crtc: crtc::Handle,
    compositor: Compositor,
    output: Output,
    /// A frame is queued and we are waiting for its vblank.
    pending: bool,
    /// The connector's resolutions, one per entry of `state.display.modes`
    /// (main screen).
    modes: Vec<Mode>,
}

struct Backend {
    drm: DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    renderer: GlesRenderer,
    formats: FormatSet,
    heads: Vec<Head>,
    /// Bumped per head created, for unique output names.
    serial: u32,
}

/// Distinct sizes, highest refresh each; the screen's preferred (native)
/// mode first, then largest first.
fn mode_list(info: &connector::Info) -> Vec<Mode> {
    let mut modes: Vec<Mode> = Vec::new();
    for m in info.modes() {
        match modes.iter_mut().find(|x| x.size() == m.size()) {
            Some(x) if x.vrefresh() < m.vrefresh() => *x = *m,
            Some(_) => {}
            None => modes.push(*m),
        }
    }
    modes.sort_by_key(|m| {
        let preferred = m.mode_type().contains(ModeTypeFlags::PREFERRED);
        (!preferred, std::cmp::Reverse(m.size().0 as i32 * m.size().1 as i32))
    });
    modes
}

fn conn_name(info: &connector::Info) -> String {
    format!("{:?}-{}", info.interface(), info.interface_id())
}

impl Backend {
    /// Match heads to the connectors that are connected now: add new
    /// screens, drop unplugged ones, lay them out left to right.
    fn scan(&mut self, state: &mut Smallvil) {
        let Ok(res) = self.drm.resource_handles() else { return };
        let connected: Vec<connector::Info> = res
            .connectors()
            .iter()
            .filter_map(|c| self.drm.get_connector(*c, false).ok())
            .filter(|i| i.state() == connector::State::Connected && !i.modes().is_empty())
            .collect();

        // Unplugged.
        let before = self.heads.len();
        self.heads.retain(|h| {
            let keep = connected.iter().any(|i| i.handle() == h.conn);
            if !keep {
                state.space.unmap_output(&h.output);
                tracing::info!(output = %h.output.name(), "display: screen removed");
            }
            keep
        });
        let removed = before != self.heads.len();

        // Plugged in.
        let cfg = crate::config::Config::load();
        let mut added = false;
        for info in &connected {
            if self.heads.iter().any(|h| h.conn == info.handle()) {
                continue;
            }
            let used: Vec<crtc::Handle> = self.heads.iter().map(|h| h.crtc).collect();
            let Some(crtc) = info
                .encoders()
                .iter()
                .filter_map(|e| self.drm.get_encoder(*e).ok())
                .flat_map(|e| res.filter_crtcs(e.possible_crtcs()))
                .find(|c| !used.contains(c))
            else {
                tracing::warn!(conn = %conn_name(info), "display: no free CRTC");
                continue;
            };
            let modes = mode_list(info);
            let main = self.heads.is_empty();
            // The main screen honours the saved resolution.
            let mode = cfg
                .resolution
                .filter(|_| main)
                .and_then(|(w, h)| modes.iter().find(|m| m.size() == (w as u16, h as u16)).copied())
                .unwrap_or(modes[0]);
            match self.add_head(state, info, crtc, mode, modes) {
                Ok(()) => added = true,
                Err(e) => tracing::warn!(?e, conn = %conn_name(info), "display: could not use screen"),
            }
        }
        if added || removed || self.heads.is_empty() {
            self.layout(state);
        }
    }

    fn add_head(
        &mut self,
        state: &mut Smallvil,
        info: &connector::Info,
        crtc: crtc::Handle,
        mode: Mode,
        modes: Vec<Mode>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let wl_mode = WlMode::from(mode);
        let (mw, mh) = info.size().unwrap_or((0, 0));
        let output = Output::new(
            format!("redrob-{}", self.serial),
            PhysicalProperties {
                size: (mw as i32, mh as i32).into(),
                subpixel: Subpixel::Unknown,
                make: "Redrob".into(),
                model: conn_name(info),
                serial_number: self.serial.to_string(),
            },
        );
        self.serial += 1;
        let _global = output.create_global::<Smallvil>(&state.display_handle);
        output.set_preferred(wl_mode);
        output.change_current_state(
            Some(wl_mode),
            Some(Transform::Normal),
            Some(smithay::output::Scale::Fractional(crate::display::scale())),
            None,
        );
        let surface = self.drm.create_surface(crtc, mode, &[info.handle()])?;
        let allocator = GbmAllocator::new(self.gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
        let exporter = GbmFramebufferExporter::new(self.gbm.clone(), NodeFilter::None);
        let compositor = DrmCompositor::new(
            &output,
            surface,
            None,
            allocator,
            exporter,
            // Opaque scan-out first: the desktop has no transparency to show through.
            [Fourcc::Xrgb8888, Fourcc::Argb8888],
            self.formats.clone(),
            self.drm.cursor_size(),
            Some(self.gbm.clone()),
        )?;
        tracing::info!(output = %output.name(), conn = %conn_name(info), size = ?mode.size(), "display: screen added");
        self.heads.push(Head { conn: info.handle(), crtc, compositor, output, pending: false, modes });
        Ok(())
    }

    /// Screens left to right, top-aligned; publish the main screen's modes
    /// and every screen's name to the settings window.
    fn layout(&mut self, state: &mut Smallvil) {
        let scale = crate::display::scale();
        let mut x = 0;
        state.display.screens.clear();
        for h in &self.heads {
            let size = h.output.current_mode().map(|m| m.size).unwrap_or_default();
            h.output.change_current_state(None, None, None, Some((x, 0).into()));
            state.space.map_output(&h.output, (x, 0));
            state.display.screens.push(format!("{} {}×{}", h.output.physical_properties().model, size.w, size.h));
            x += (size.w as f64 / scale).round() as i32;
        }
        if let Some(main) = self.heads.first() {
            let cur = main.output.current_mode().map(|m| m.size).unwrap_or_default();
            state.display.name = main.output.physical_properties().model;
            state.display.modes = main.modes.iter().map(|m| (m.size().0 as i32, m.size().1 as i32, WlMode::from(*m).refresh)).collect();
            state.display.current = main.modes.iter().position(|m| (m.size().0 as i32, m.size().1 as i32) == (cur.w, cur.h)).unwrap_or(0);
        }
        // Windows stranded on a screen that went away come back to the main one.
        let area: Vec<Rectangle<i32, Logical>> = state.space.outputs().filter_map(|o| state.space.output_geometry(o)).collect();
        let stranded: Vec<_> = state
            .space
            .elements()
            .filter(|w| {
                let Some(loc) = state.space.element_location(w) else { return false };
                !area.iter().any(|a| a.contains(loc + Point::from((20, 20))))
            })
            .cloned()
            .collect();
        for w in stranded {
            state.space.map_element(w, (80, 80), false);
        }
    }

    fn head_by_crtc(&self, c: crtc::Handle) -> Option<usize> {
        self.heads.iter().position(|h| h.crtc == c)
    }
}

pub fn init_udev(event_loop: &mut EventLoop<Smallvil>, state: &mut Smallvil) -> Result<(), Box<dyn std::error::Error>> {
    let (mut session, notifier) = LibSeatSession::new()?;
    let seat = session.seat();

    let gpu = primary_gpu(&seat)?
        .or_else(|| all_gpus(&seat).ok().and_then(|g| g.into_iter().next()))
        .ok_or("no GPU found")?;
    tracing::info!(?gpu, "using GPU");
    let fd = session.open(&gpu, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK)?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm, drm_notifier) = DrmDevice::new(fd.clone(), true)?;
    let gbm = GbmDevice::new(fd)?;

    let display = unsafe { EGLDisplay::new(gbm.clone())? };
    let context = EGLContext::new(&display)?;
    let renderer = unsafe { GlesRenderer::new(context)? };
    let formats = renderer.dmabuf_formats();

    if let Some(s) = crate::config::Config::load().scale {
        crate::display::set_scale(s);
    }
    let backend = Rc::new(RefCell::new(Backend { drm, gbm, renderer, formats, heads: Vec::new(), serial: 0 }));
    backend.borrow_mut().scan(state);
    if backend.borrow().heads.is_empty() {
        return Err("no connected output".into());
    }

    // Scan-out finished: release the buffer and draw that screen's next frame.
    let b = backend.clone();
    event_loop.handle().insert_source(drm_notifier, move |event, _, state| match event {
        DrmEvent::VBlank(c) => {
            let mut be = b.borrow_mut();
            if let Some(i) = be.head_by_crtc(c) {
                let _ = be.heads[i].compositor.frame_submitted();
                be.heads[i].pending = false;
                drop(be);
                render(&b, state, i);
            }
        }
        DrmEvent::Error(e) => tracing::error!(?e, "drm error"),
    })?;

    // Screens plugged in or out (a "change" uevent on the GPU).
    let b = backend.clone();
    let udev = UdevBackend::new(&seat)?;
    event_loop.handle().insert_source(udev, move |event, _, state| {
        if let UdevEvent::Changed { .. } = event {
            b.borrow_mut().scan(state);
        }
    })?;

    // Input.
    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));
    libinput.udev_assign_seat(&seat).map_err(|_| "libinput seat")?;
    event_loop
        .handle()
        .insert_source(LibinputInputBackend::new(libinput.clone()), move |event, _, state| {
            state.process_input_event(event);
        })?;

    // VT switch: pause/resume libinput with the session.
    let mut li = libinput.clone();
    event_loop.handle().insert_source(notifier, move |event, _, _| match event {
        SessionEvent::PauseSession => {
            tracing::info!("session paused");
            li.suspend()
        }
        SessionEvent::ActivateSession => {
            tracing::info!("session activated");
            let _ = li.resume();
        }
    })?;
    // The session can still be activating when we open the input devices,
    // which leaves them paused. Re-open them once we are settled.
    tracing::info!(active = session.is_active(), "session at start");
    let mut li = libinput.clone();
    event_loop.handle().insert_source(Timer::from_duration(Duration::from_millis(1500)), move |_, _, _| {
        li.suspend();
        let r = li.resume();
        tracing::info!(ok = r.is_ok(), "input devices re-opened");
        TimeoutAction::Drop
    })?;

    // Keep the session alive for the whole run.
    std::mem::forget(session);

    // A timer for frames with no damage (nothing queued -> no vblank).
    let b = backend.clone();
    event_loop.handle().insert_source(Timer::immediate(), move |_, _, state| {
        let n = b.borrow().heads.len();
        for i in 0..n {
            render(&b, state, i);
        }
        TimeoutAction::ToDuration(Duration::from_millis(16))
    })?;
    Ok(())
}

/// Apply a resolution or scale picked in the settings window.
fn apply_request(be: &mut Backend, state: &mut Smallvil) {
    let Some(req) = state.display.request.take() else { return };
    match req {
        crate::display::Request::Mode(i) => {
            let Some(h) = be.heads.first_mut() else { return };
            let Some(m) = h.modes.get(i).copied() else { return };
            let before = h.output.current_mode().map(|m| m.size).unwrap_or_default();
            match h.compositor.use_mode(m) {
                Ok(()) => {
                    h.output.change_current_state(Some(WlMode::from(m)), None, None, None);
                    h.compositor.reset_buffers();
                    // Ask to keep it; it is saved only when kept.
                    let back = h.modes.iter().position(|x| (x.size().0 as i32, x.size().1 as i32) == (before.w, before.h));
                    state.confirm_resolution(back, (m.size().0 as i32, m.size().1 as i32));
                    tracing::info!(size = ?m.size(), "display: resolution");
                }
                Err(e) => tracing::warn!(?e, "display: mode change failed"),
            }
        }
        crate::display::Request::Revert(i) => {
            let Some(h) = be.heads.first_mut() else { return };
            if let Some(m) = h.modes.get(i).copied() {
                if h.compositor.use_mode(m).is_ok() {
                    h.output.change_current_state(Some(WlMode::from(m)), None, None, None);
                    h.compositor.reset_buffers();
                    tracing::info!(size = ?m.size(), "display: resolution reverted");
                }
            }
        }
        crate::display::Request::Scale(s) => {
            crate::display::set_scale(s);
            for h in &mut be.heads {
                h.output.change_current_state(None, None, Some(smithay::output::Scale::Fractional(s)), None);
                h.compositor.reset_buffers();
            }
            crate::config::set_value("scale", &format!("{s}"));
            state.chrome.invalidate();
            state.deco.invalidate();
            state.layers.invalidate();
            tracing::info!(s, "display: scale");
        }
    }
    be.layout(state);
}

fn render(b: &Rc<RefCell<Backend>>, state: &mut Smallvil, i: usize) {
    let mut be = b.borrow_mut();
    if i == 0 {
        apply_request(&mut be, state);
    }
    let be = &mut *be;
    let Some(h) = be.heads.get_mut(i) else { return };
    if h.pending {
        return;
    }
    // Damage tracking (partial repaint) by default; REDROB_FULL_REPAINT=1
    // repaints every frame for scan-out devices whose buffer ages lie
    // (QEMU's bochs).
    if std::env::var_os("REDROB_FULL_REPAINT").is_some() {
        h.compositor.reset_buffer_ages();
    }
    let size = h.output.current_mode().unwrap().size;
    let origin = state.space.output_geometry(&h.output).map(|g| g.loc).unwrap_or_default();

    let elements = if i == 0 {
        crate::scene::take_shot(state, &mut be.renderer, size);
        crate::scene::elements(state, &mut be.renderer, size, true)
    } else {
        crate::scene::elements_secondary(state, &mut be.renderer, origin, size)
    };

    if i == 0 && state.chrome.frame_log_due() {
        tracing::info!(n = elements.len(), screens = be.heads.len(), "frame");
    }
    let h = &mut be.heads[i];
    match h.compositor.render_frame(&mut be.renderer, &elements, state.chrome.ground(), FrameFlags::DEFAULT) {
        Ok(r) if !r.is_empty => match h.compositor.queue_frame(()) {
            Ok(()) => h.pending = true,
            Err(e) => tracing::warn!(?e, "queue_frame"),
        },
        Ok(_) => {}
        Err(e) => tracing::warn!(?e, "render_frame"),
    }

    let out = h.output.clone();
    state.space.elements().for_each(|w| {
        w.send_frame(&out, state.start_time.elapsed(), Some(Duration::ZERO), |_, _| Some(out.clone()))
    });
    state.space.refresh();
    state.popups.cleanup();
    let _ = state.display_handle.flush_clients();
}
