# Display module, kiosk mode (dev3, QEMU virtio-gpu)

Date: 2026-10-09. Image: `redrob_generic-x86-64-0.1.dev3` (first build with `redrob-display`
and DejaVu Sans). Harness: `scripts/ota-harness/t10_display.py`, same VM recipe as `ota.md`
plus `-device virtio-gpu-pci -device qemu-xhci -device usb-kbd`.

## Design decision: KMS dumb buffers, not cage

The plan said "cage + dashboard UI". Cage needs wlroots, which needs Mesa (EGL/GLES and, on
a machine without a GPU, llvmpipe and therefore LLVM) in the image. That is a multi-hour
build, a large surface, and nothing the dashboard needs. Kiosk mode is instead one Rust
binary (`modules/display/kiosk`, 977 KB) that:

- opens the first DRM card with a connected connector, picks its preferred mode, allocates a
  dumb buffer and `set_crtc`s it (no Mesa, no compositor; works on virtio-gpu, bochs, and
  any KMS driver);
- renders the dashboard with tiny-skia + fontdue (DejaVu Sans from the image): service
  states, input devices held, uptime, the pairing device/host/address/code and the same
  `redrob://pair` QR the login banner shows (from `/run/redrob-pairing/pairing.json`);
- opens every `/dev/input/event*` and grabs it (`EVIOCGRAB`), so no other process sees
  keyboards or mice while the kiosk is up.

`desktop` mode (cage/wlroots) stays in `module.yaml` as planned, not shipped.

## Isolation: the module is the only holder of /dev/dri and /dev/input

| Mechanism | Effect |
|---|---|
| `redrob-display` user, sole member of `video` and `input` | `/dev/dri/card*` is `root:video 0660`, `/dev/input/event*` is `root:input 0660` (udev defaults); `redrob-agent`, brokers, pairing cannot open them |
| unit: `DevicePolicy=closed` + `DeviceAllow=char-drm rw` + `DeviceAllow=char-input rw` | even as that user, only those two device classes |
| other units: `PrivateDevices=yes` | the agent and brokers see no real device nodes at all |
| `Conflicts=getty@tty1.service` | starting the kiosk stops the VT1 login; stopping it brings the getty back |
| `CapabilityBoundingSet=` (empty), `PrivateNetwork=yes`, `ProtectSystem=strict` | the kiosk cannot reach the network or write anywhere |

## Mode switch

`/mnt/data/redrob/display/mode` holds `headless` (or is absent, the default) or `kiosk`.
The unit is enabled but has `ConditionPathExists` on the file and an `ExecCondition` that
checks for `kiosk`, so headless devices skip it silently at every boot. Switching is:

```sh
mkdir -p /mnt/data/redrob/display && echo kiosk > /mnt/data/redrob/display/mode
systemctl restart redrob-display.service
```

No reboot; the mode persists on `/mnt/data`.

## Automated: `cargo test` -- 5 tests, 0 failures

pairing JSON parse + URI equality with the pairing tool; rendered frame has the accent rule,
a QR quiet zone and is not mostly background; XRGB blit byte order and pitch; PPM header;
uptime text.

## T10 (guest)

Run 2026-10-09 on dev3, 17/17 checks pass (`t10.json`). The disk is booted with
`console=tty0 console=ttyS0,115200` injected into the ESP `cmdline.txt` (the shipped image
ships a `tty0`-only cmdline; the serial line is a test-harness need, not a product change).

| Check | Result |
|---|---|
| headless: unit skipped, `ConditionResult=no`; a login console is up (serial getty on `ttyS0` here, `getty@tty1` on a VGA-only unit); screendump 1 is the text console | PASS |
| write `kiosk` + `systemctl start` -> unit `active`, process user `redrob-display` | PASS |
| `getty@tty1` not active while kiosk holds the display (`Conflicts`) | PASS |
| screendump 2: accent rule `#c162f4` present (4884 px), QR quiet zone present (66297 white px), differs from screendump 1 | PASS |
| `/dev/dri/card0` `root:video 660`, `/dev/input/event0` `root:input 660`, only `redrob-display` in `video`/`input` | PASS |
| `systemd-run --uid=redrob-agent` cannot open `/dev/dri/card0` nor `/dev/input/event0` (both `rc=1`) | PASS |
| journal shows the kiosk grabbed all 4 input devices | PASS |
| reboot: kiosk comes back on its own (`active`), getty stays down; mode file persists on `/mnt/data` | PASS |

The clean `systemctl reboot` over the serial console stalls on shutdown (a known serial-tty
quirk; `reboot: "HUNG; hard reset"`), so the harness power-cycles the VM. The kiosk still
returns on its own after that hard cycle, which is the stronger evidence for persistence.

Screenshots: `~/recordings/redrob-os-t10-headless.png`, `~/recordings/redrob-os-t10-kiosk.png`,
`~/recordings/redrob-os-t10-kiosk-reboot.png`.

## Not covered here

- A real GPU/monitor (QEMU virtio-gpu only), multi-connector layouts, hotplug of the display.
- Touch or pointer interaction: the kiosk holds the devices but draws no cursor and has no
  controls yet; the dashboard is read-only.
- Desktop mode.
