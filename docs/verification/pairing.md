# First-boot pairing and boot branding (dev2, QEMU TCG)

Date: 2026-10-09. Image: `redrob_generic-x86-64-0.1.dev2` (first build with `redrob-pairing`,
the kernel logo and a plain getty on VT1). Harness: `scripts/ota-harness/t9_pairing.py`
(same VM recipe as `ota.md`, plus `-device VGA` so fbcon paints the logo and the monitor can
`screendump` it).

## What ships

| Piece | Where | Note |
|---|---|---|
| `redrob-pairing` (Rust, `tools/redrob-pairing`) | `/usr/bin/redrob-pairing` | reads device id, local addresses, the agent's one-time code (`GET /admin/paircode` on loopback with the admin token); writes `/run/issue.d/50-redrob-pairing.issue` (text + QR of `redrob://pair?...`) and `/run/redrob-pairing/pairing.json` for the display module |
| `redrob-pairing.timer` | 20 s after boot, then every 30 s | the code is one-time and changes after pairing, so the banner is refreshed, not written once |
| `redrob-pairing.service` | oneshot, root, `CAP_DAC_READ_SEARCH` only, `ProtectSystem=strict`, own `RuntimeDirectory` | retries the agent for ~16 s on a cold start instead of printing an ambiguous banner |
| VT1 getty | `redrob-post-build.sh` masks `ha-cli@tty1`, enables `getty@tty1` | the console shows `/etc/issue` + the banner like the serial port |
| Boot logo | `BR2_LINUX_KERNEL_CUSTOM_LOGO_PATH` -> `branding/boot-logo.png` (`CONFIG_LOGO`, `CLUT224`) | fbcon paints it top-left during boot; a full-screen splash is the display module's job |

Headless devices: the same code is exposed over the agent's channels; the serial login prompt
shows the full banner.

## Automated: `cargo test` -- 3 tests, 0 failures

QR renders and the `redrob://pair` URI is stable; banner with and without a code; JSON key
extraction from the agent reply.

## T9 (guest)

Run 1 exposed two defects, fixed before run 2:

- the first timer run fired while the agent was still starting and printed the
  "already paired, or the agent is still starting" line for a whole period (now retried);
- the unit claimed `RuntimeDirectory=redrob`, which is the credential broker's socket
  directory (0750, broker user) and would have re-owned it to root on every run. The JSON now
  lives in `/run/redrob-pairing/`.

Run 2 results:

| Check | Result |
|---|---|
| `redrob-pairing.timer` active, service `Result=success` | pass (`Result=success`, `ExecMainStatus=0`) |
| banner has device id, `addr:port`, a code, a QR block | pass (32-char code, QR present) |
| `pairing.json` parses, device id and code match the banner | pass |
| banner printed above the serial `login:` prompt | pass |
| `getty@tty1` enabled, `ha-cli@tty1` masked | pass (`enabled masked`) |
| `/run/redrob` still owned by the broker user | pass (`redrob-broker:redrob-broker 750`) |
| `CONFIG_LOGO=y`, `CONFIG_LOGO_LINUX_CLUT224=y` in `/proc/config.gz` | pass |
| VGA screendump: top-left 96x96 tile non-black (mean 0.047) | pass |
| `os-release` `NAME="Redrob OS"`, `VERSION="0.1.dev2 (Generic x86-64)"` | pass |

Screenshot: `~/recordings/redrob-os-t9-vga.png`.

## Not covered here

- The phone side of pairing (scanning the QR, redeeming the code) needs the console and a
  real channel; this machine verifies only that the device exposes a correct, refreshing code.
- Boot logo on real firmware/GOP framebuffers (QEMU VGA only).
