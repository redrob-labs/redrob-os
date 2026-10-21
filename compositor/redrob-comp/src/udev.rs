//! Real-hardware backend: libseat session, udev GPU, DRM/KMS scan-out through
//! Smithay's DrmCompositor, libinput for keyboard/mouse/tablet. One GPU, one
//! connected output (the VM's virtual screen, a laptop panel). Software EGL
//! (llvmpipe on a KMS-only device such as QEMU's bochs) is accepted on purpose.
//! Structure follows Smithay's `anvil` udev backend (MIT), cut down to one output.

use std::{cell::RefCell, rc::Rc, time::Duration};

use smithay::{
    backend::{
        allocator::{
            Fourcc,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{
            DrmDevice, DrmDeviceFd, DrmEvent,
            compositor::{DrmCompositor, FrameFlags},
            exporter::gbm::{GbmFramebufferExporter, NodeFilter},
        },
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            ImportDma,
            element::{
                AsRenderElements, memory::MemoryRenderBufferRenderElement,
                surface::WaylandSurfaceRenderElement,
            },
            gles::GlesRenderer,
        },
        session::{Event as SessionEvent, Session, libseat::LibSeatSession},
        udev::{all_gpus, primary_gpu},
    },
    output::{Mode as WlMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{
            EventLoop,
            timer::{TimeoutAction, Timer},
        },
        drm::control::{Device as ControlDevice, ModeTypeFlags, connector},
        input::Libinput,
        rustix::fs::OFlags,
    },
    utils::{DeviceFd, Scale, Transform},
};

use crate::Smallvil;

smithay::backend::renderer::element::render_elements! {
    pub ShellElement<=GlesRenderer>;
    Window=WaylandSurfaceRenderElement<GlesRenderer>,
    Memory=MemoryRenderBufferRenderElement<GlesRenderer>,
}

type Compositor =
    DrmCompositor<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

struct Drm {
    renderer: GlesRenderer,
    compositor: Compositor,
    output: Output,
    /// A frame is queued and we are waiting for its vblank.
    pending: bool,
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
    let (mut drm, drm_notifier) = DrmDevice::new(fd.clone(), true)?;
    let gbm = GbmDevice::new(fd)?;

    let display = unsafe { EGLDisplay::new(gbm.clone())? };
    let context = EGLContext::new(&display)?;
    let renderer = unsafe { GlesRenderer::new(context)? };

    // First connected connector, its preferred mode, the first CRTC its encoder can drive.
    let res = drm.resource_handles()?;
    let (conn, crtc, mode) = res
        .connectors()
        .iter()
        .filter_map(|c| drm.get_connector(*c, false).ok())
        .filter(|i| i.state() == connector::State::Connected)
        .find_map(|info| {
            let crtc = info
                .encoders()
                .iter()
                .filter_map(|e| drm.get_encoder(*e).ok())
                .flat_map(|e| res.filter_crtcs(e.possible_crtcs()))
                .next()?;
            let mode = info
                .modes()
                .iter()
                .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
                .or_else(|| info.modes().first())
                .copied()?;
            Some((info.handle(), crtc, mode))
        })
        .ok_or("no connected output")?;

    let wl_mode = WlMode::from(mode);
    let output = Output::new(
        "redrob-0".into(),
        PhysicalProperties { size: (0, 0).into(), subpixel: Subpixel::Unknown, make: "Redrob".into(), model: "DRM".into(), serial_number: "0".into() },
    );
    let _global = output.create_global::<Smallvil>(&state.display_handle);
    output.set_preferred(wl_mode);
    output.change_current_state(Some(wl_mode), Some(Transform::Normal), None, Some((0, 0).into()));
    state.space.map_output(&output, (0, 0));

    let surface = drm.create_surface(crtc, mode, &[conn])?;
    let allocator = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
    let exporter = GbmFramebufferExporter::new(gbm.clone(), NodeFilter::None);
    let renderer_formats = renderer.dmabuf_formats();
    let compositor = DrmCompositor::new(
        &output,
        surface,
        None,
        allocator,
        exporter,
        [Fourcc::Argb8888, Fourcc::Xrgb8888],
        renderer_formats,
        drm.cursor_size(),
        Some(gbm),
    )?;

    let drm_state = Rc::new(RefCell::new(Drm { renderer, compositor, output, pending: false }));

    // Scan-out finished: release the buffer and draw the next frame.
    let d = drm_state.clone();
    event_loop.handle().insert_source(drm_notifier, move |event, _, state| match event {
        DrmEvent::VBlank(_) => {
            let mut drm = d.borrow_mut();
            let _ = drm.compositor.frame_submitted();
            drm.pending = false;
            drop(drm);
            render(&d, state);
        }
        DrmEvent::Error(e) => tracing::error!(?e, "drm error"),
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
    // The autologin session can still be activating when we open the input
    // devices, which leaves them paused. Re-open them once we are settled.
    tracing::info!(active = session.is_active(), "session at start");
    let mut li = libinput.clone();
    event_loop.handle().insert_source(Timer::from_duration(Duration::from_millis(1500)), move |_, _, _| {
        li.suspend();
        let r = li.resume();
        tracing::info!(ok = r.is_ok(), "input devices re-opened");
        TimeoutAction::Drop
    })?;

    // Keep the DRM device alive for the whole session.
    std::mem::forget(drm);
    std::mem::forget(session);

    // A timer source for frames with no damage (nothing queued -> no vblank).
    let d = drm_state.clone();
    event_loop.handle().insert_source(Timer::immediate(), move |_, _, state| {
        render(&d, state);
        TimeoutAction::ToDuration(Duration::from_millis(16))
    })?;
    Ok(())
}

fn render(d: &Rc<RefCell<Drm>>, state: &mut Smallvil) {
    let mut drm = d.borrow_mut();
    if drm.pending {
        return;
    }
    let Drm { renderer, compositor, output, pending, .. } = &mut *drm;
    let size = output.current_mode().unwrap().size;

    // Front to back: cursor, bar + dock, then windows top-most first.
    let mut elements: Vec<ShellElement> = Vec::new();
    let pointer = state.seat.get_pointer().unwrap().current_location();
    if let Some(c) = state.chrome.cursor_element(renderer, pointer) {
        elements.push(ShellElement::Memory(c));
    }
    elements.extend(state.chrome.elements(renderer, size).into_iter().map(ShellElement::Memory));
    for window in state.space.elements().rev() {
        let Some(loc) = state.space.element_location(window) else { continue };
        let w: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
            window.render_elements(renderer, loc.to_physical(1), Scale::from(1.0), 1.0);
        elements.extend(w.into_iter().map(ShellElement::Window));
    }
    if let Some(g) = state.chrome.ground_element(renderer, size) {
        elements.push(ShellElement::Memory(g));
    }

    match compositor.render_frame(renderer, &elements, state.chrome.ground(), FrameFlags::DEFAULT) {
        Ok(r) if !r.is_empty => match compositor.queue_frame(()) {
            Ok(()) => *pending = true,
            Err(e) => tracing::warn!(?e, "queue_frame"),
        },
        Ok(_) => {}
        Err(e) => tracing::warn!(?e, "render_frame"),
    }

    let out = output.clone();
    drop(drm);
    state.space.elements().for_each(|w| {
        w.send_frame(&out, state.start_time.elapsed(), Some(Duration::ZERO), |_, _| Some(out.clone()))
    });
    state.space.refresh();
    state.popups.cleanup();
    let _ = state.display_handle.flush_clients();
}
