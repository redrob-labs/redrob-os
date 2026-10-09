//! `redrob-display`: kiosk mode of the display module.
//!
//! Scanout is a KMS dumb buffer on the first connected connector of the first DRM card:
//! no Mesa, no compositor, so it runs on virtio-gpu/bochs in QEMU and on any KMS driver.
//! All `/dev/input/event*` devices are opened and grabbed (EVIOCGRAB) so nothing else on
//! the box sees keyboards or mice while the kiosk is up. `--once FILE.ppm` renders one
//! frame to a file instead (host tests, harness).

use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use drm::Device;
use drm::buffer::DrmFourcc;
use drm::control::{Device as ControlDevice, connector, crtc};
use redrob_display::{Renderer, Status, blit_xrgb, write_ppm};

struct Card(std::fs::File);
impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl Device for Card {}
impl ControlDevice for Card {}

struct Opts {
    card: Option<PathBuf>,
    font: PathBuf,
    pairing: PathBuf,
    interval: Duration,
    once: Option<PathBuf>,
    size: (u32, u32),
    grab_input: bool,
}

fn usage() -> ! {
    eprintln!(
        "usage: redrob-display [--card /dev/dri/cardN] [--font FILE.ttf] [--pairing FILE.json]\n\
         \x20                     [--interval SECS] [--no-input] [--once OUT.ppm [--size WxH]]"
    );
    std::process::exit(2)
}

fn parse() -> Opts {
    let mut o = Opts {
        card: None,
        font: PathBuf::from("/usr/share/fonts/dejavu/DejaVuSans.ttf"),
        pairing: PathBuf::from("/run/redrob-pairing/pairing.json"),
        interval: Duration::from_secs(2),
        once: None,
        size: (1280, 800),
        grab_input: true,
    };
    let mut a = std::env::args().skip(1);
    while let Some(k) = a.next() {
        let mut v = || a.next().unwrap_or_else(|| usage());
        match k.as_str() {
            "--card" => o.card = Some(PathBuf::from(v())),
            "--font" => o.font = PathBuf::from(v()),
            "--pairing" => o.pairing = PathBuf::from(v()),
            "--interval" => {
                o.interval = Duration::from_secs(v().parse().unwrap_or_else(|_| usage()))
            }
            "--once" => o.once = Some(PathBuf::from(v())),
            "--no-input" => o.grab_input = false,
            "--size" => {
                let s = v();
                let (w, h) = s.split_once('x').unwrap_or_else(|| usage());
                o.size = (
                    w.parse().unwrap_or_else(|_| usage()),
                    h.parse().unwrap_or_else(|_| usage()),
                );
            }
            _ => usage(),
        }
    }
    o
}

/// Grab every evdev device we can open; returns the live count (devices stay open in a
/// background thread that drains their events so the kernel queues do not fill up).
fn grab_inputs() -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let mut devs = Vec::new();
    for (path, mut dev) in evdev::enumerate() {
        match dev.grab() {
            Ok(()) => {
                eprintln!(
                    "redrob-display: grabbed {} ({})",
                    path.display(),
                    dev.name().unwrap_or("?")
                );
                let _ = dev.set_nonblocking(true);
                devs.push(dev);
            }
            Err(e) => eprintln!("redrob-display: grab {}: {e}", path.display()),
        }
    }
    count.store(devs.len(), Ordering::Relaxed);
    let c = Arc::clone(&count);
    std::thread::spawn(move || {
        loop {
            for d in devs.iter_mut() {
                if let Ok(events) = d.fetch_events() {
                    for _ in events {}
                }
            }
            c.store(devs.len(), Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    count
}

fn pick_card(
    explicit: Option<&Path>,
) -> Result<(Card, connector::Info, crtc::Handle, drm::control::Mode)> {
    let candidates: Vec<PathBuf> = match explicit {
        Some(p) => vec![p.to_path_buf()],
        None => {
            let mut v: Vec<PathBuf> = std::fs::read_dir("/dev/dri")
                .context("list /dev/dri")?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("card"))
                })
                .collect();
            v.sort();
            v
        }
    };
    for path in candidates {
        let Ok(file) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
        else {
            continue;
        };
        let card = Card(file);
        let Ok(res) = card.resource_handles() else {
            continue;
        };
        for &ch in res.connectors() {
            let Ok(conn) = card.get_connector(ch, false) else {
                continue;
            };
            if conn.state() != connector::State::Connected || conn.modes().is_empty() {
                continue;
            }
            let mode = conn
                .modes()
                .iter()
                .find(|m| {
                    m.mode_type()
                        .contains(drm::control::ModeTypeFlags::PREFERRED)
                })
                .or_else(|| conn.modes().first())
                .copied()
                .expect("non-empty");
            // CRTC: the one the connector's encoder is on, else any the encoder may use.
            let mut crtc_h = None;
            for &eh in conn.encoders() {
                if let Ok(enc) = card.get_encoder(eh) {
                    crtc_h = enc
                        .crtc()
                        .or_else(|| res.filter_crtcs(enc.possible_crtcs()).first().copied());
                    if crtc_h.is_some() {
                        break;
                    }
                }
            }
            let Some(crtc_h) = crtc_h else { continue };
            eprintln!(
                "redrob-display: {} connector {:?} mode {}x{}@{}",
                path.display(),
                conn.interface(),
                mode.size().0,
                mode.size().1,
                mode.vrefresh()
            );
            return Ok((card, conn, crtc_h, mode));
        }
    }
    bail!("no DRM card with a connected connector")
}

fn main() -> Result<()> {
    let o = parse();
    let renderer = Renderer::from_file(&o.font)?;

    if let Some(out) = &o.once {
        let st = Status::collect(&o.pairing, 0);
        let pm = renderer.render(&st, o.size.0, o.size.1);
        write_ppm(&pm, out)?;
        return Ok(());
    }

    let (card, conn, crtc_h, mode) = pick_card(o.card.as_deref())?;
    let (w, h) = (mode.size().0 as u32, mode.size().1 as u32);
    let mut db = card
        .create_dumb_buffer((w, h), DrmFourcc::Xrgb8888, 32)
        .context("create dumb buffer")?;
    let fb = card
        .add_framebuffer(&db, 24, 32)
        .context("add framebuffer")?;
    card.set_crtc(crtc_h, Some(fb), (0, 0), &[conn.handle()], Some(mode))
        .context("set crtc (is another process DRM master?)")?;
    let pitch = drm::buffer::Buffer::pitch(&db) as usize;

    let inputs = if o.grab_input {
        grab_inputs()
    } else {
        Arc::new(AtomicUsize::new(0))
    };

    loop {
        let st = Status::collect(&o.pairing, inputs.load(Ordering::Relaxed));
        let pm = renderer.render(&st, w, h);
        {
            let mut map = card.map_dumb_buffer(&mut db).context("map dumb buffer")?;
            blit_xrgb(&pm, map.as_mut(), pitch);
        }
        // Dumb buffers are not implicitly flushed on every driver (bochs/virtio-gpu need
        // the dirty-fb hint); re-setting the same CRTC is the portable way to kick it.
        let _ = card.set_crtc(crtc_h, Some(fb), (0, 0), &[conn.handle()], Some(mode));
        std::thread::sleep(o.interval);
    }
}
