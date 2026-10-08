# USB broker verification (QEMU, real guest kernel)

Date: 2026-10-08. Binary: `modules/usb-broker/broker` built for `x86_64-unknown-linux-musl`
(static, 3.7 MB) and dropped into a running dev1 guest under `/mnt/data`, because the image
was not rebuilt for this stage. Kernel `6.18.55-haos`, QEMU `qemu-xhci`, devices hot-plugged
with `device_add` on the monitor. Unit `redrob-usb-broker.service` verified with
`systemd-analyze` only; it ships with the next build.

## Mechanism

No USBGuard package: the kernel's own USB authorization is the enforcement point. The broker
writes `authorized_default=0` on every host controller at start, so a new device gets no
driver until it writes `authorized=1` for it. Devices present at boot stay as the kernel left
them (`trust_boot_devices`). Interfaces of an unauthorized device are classified from the raw
`descriptors` blob, so a BadUSB stick is recognised before anything binds.

## Automated: `cargo test` -- 13 tests, 0 failures

| Area | Checks |
|---|---|
| policy | storage + HID on one device -> `Reject` even when allow-listed; plain storage `Pending` -> `Allow` after approval; `vendor:product:` entry covers any serial; keyboard pends, hub allows, deny beats allow |
| descriptors | interface descriptors parsed from the raw blob (classes 08 + 03 found); truncated/garbage blobs do not panic |
| sysfs | device read from a fixture tree; identity `vid:pid:serial` |
| broker | `authorized_default` 1 -> 0; boot device trusted; hot-plugged keyboard `authorized=0` until approve, `1` after, remembered in `policy.json`, allowed on replug; BadUSB stays `0` and `approve` is refused; a block device of a pending device is not mounted; after approval it is, under `/run/media/redrob/<label>`; a label with `..` falls back to the node name; deny unmounts and writes `authorized=0`; audit has mount/umount/deny |
| udev | `udevadm monitor --property` blocks parsed; DEVPATH -> usb_device name and block -> usb ancestor |

## QEMU end to end (`t8_usb.py` in the OTA harness directory)

| Step | Observed |
|---|---|
| start | `authorized_default` `1 1` -> `0 0` on usb1/usb2 |
| `device_add usb-kbd` | broker: `pending / awaiting user approval`; sysfs `authorized=0`; no HID interface |
| `POST /v1/devices/1-1/approve {remember:true}` | `authorized=1`; `/dev/input/event0..3` appear; `policy.json` gains `0627:0001:<serial>` |
| `device_add usb-storage` (16 MiB FAT, label `REDROBSTK`, one file) | `pending`; **no `/dev/sd*`** |
| approve (once) | `/dev/sda` appears; mounted `/run/media/redrob/REDROBSTK` `ro,nosuid,nodev,noexec`; `cat hello.txt` reads the file; `echo > evil.txt` fails (`rc=1`) |
| `device_del stick1` | mount gone, device record gone, audit `umount` + `remove` |
| replug keyboard | `pending` again -- QEMU's keyboard serial embeds the port path (`...03.0-1` vs `...03.0-3`), so the identity changes; a real device with a fixed serial, or one without a serial (`vid:pid:` match), is remembered |

Audit (`/mnt/data/redrob/audit/usb-YYYY-MM-DD.jsonl`) carried every step above with vendor,
product, classes and the decision.

## Not covered

- BadUSB on real hardware: QEMU cannot present a composite storage+HID device; the classifier
  is unit-tested on a synthetic descriptor blob only.
- The approval UI: `POST .../approve` was issued by hand; the agent/dashboard side that asks
  the user is Stage 5/6 work.
- `mount-failed` is logged for a partition node (`sda1`) the kernel invents on a
  superfloppy image; harmless, the whole-disk mount succeeded.
- Unit on the device (`CapabilityBoundingSet`, `ReadWritePaths=/sys/bus/usb`), needs the
  next image build.
