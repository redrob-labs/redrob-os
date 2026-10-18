# Redrob OS

Redrob OS is an agentic desktop operating system for PCs, laptops and the Raspberry Pi 5.
A resident agent works for you inside a sandbox, connected to the channels you already
use (Slack, Discord, Google), while the desktop itself stays out of your way: it updates
itself in the background and rolls back on its own if an update does not boot.

한국어: [README.ko.md](./README.ko.md)

## Install

Download the installer ISO from
[Releases](https://github.com/redrob-labs/redrob-os/releases/latest), write it to a USB
stick, boot it and follow the guided installer (disk selection, dual boot with Windows,
disk encryption, your user account). Raspberry Pi 5 uses a raw disk image instead.
Full steps: [docs/install.md](./docs/install.md).

## Desktop

A familiar desktop: taskbar, start menu, system tray and a Settings app. Apps come from the **Discover** store (Flathub) and update themselves. The
**Redrob Launcher** starts at login and installs the Redrob apps (Browser, Office,
Canvas, Code) into your home directory. Korean input is built in (Shift+Space or the
한/영 key). Tour: [docs/desktop.md](./docs/desktop.md).

## Updates and rollback

The whole OS is one signed image. The device checks for a new image once a day,
stages it in the background and switches to it at the next reboot. A boot health check
runs after every update; if it fails, the device boots the previous image by itself. You
can also pick the previous version from the boot menu at any time. Details:
[docs/maintenance.md](./docs/maintenance.md) and
[docs/decisions/0001-bootc-base.md](./docs/decisions/0001-bootc-base.md).

## Agent

The agent runs as a system service and executes tasks (including coding) in a sandbox
that cannot see your credentials or the rest of the machine. First boot shows a QR
code to pair your phone; after that you talk to it over your channels. How it is wired
into the image: [docs/agent-on-bootc.md](./docs/agent-on-bootc.md). Reaching the device
from a phone, on the same Wi‑Fi or through a relay you run:
[docs/remote-access.md](./docs/remote-access.md).

## Monorepo layout

| Path | Role |
|---|---|
| `image/` | The OS image: `Containerfile`, base pin (`base.env`), artifact pins (`pins.env`), file overlays (`files/`, `agent/`, `branding/`), build scripts and installer config (`installer/`) |
| `agent/` | Resident agent runtime: channels, cron, secrets, tool sandbox client (vendored, see below) |
| `tools/` | Small on-device helpers (`redrob-pairing`: first-boot pairing QR) |
| `modules/` | Optional on-device services: `credential-broker`, `usb-broker`, `local-inference` |
| `docs/` | Install and desktop guides, requirements, design notes, decisions (ADRs), maintenance |
| `.github/` | CI: build, sign and publish the image; build the installer on release tags; dependency bumps |

Redrob Code (the coding tool that runs inside the sandbox) lives in its own repository,
[redrob-labs/redrob-code](https://github.com/redrob-labs/redrob-code).

## Upstream tracking

Two upstreams, pinned in one place each and recorded in [UPSTREAM.md](./UPSTREAM.md) and
[`upstream-base.json`](./upstream-base.json):

| What | How it is tracked |
|---|---|
| `agent/` | `git subtree` with full history from [zeroclaw-labs/zeroclaw](https://github.com/zeroclaw-labs/zeroclaw); synced tag by tag |
| Base OS image | An OCI image reference in `image/base.env`: [Fedora Kinoite](https://fedoraproject.org/atomic-desktops/kinoite/) (`quay.io/fedora-ostree-desktops/kinoite`), which supplies the kernel, the KDE Plasma desktop and bootc; the tag is bumped by Renovate pull requests |

Everything else the image contains comes from the base distribution's own package
repositories and is listed in `NOTICE`.

## License

Apache-2.0. See `LICENSE` and `NOTICE`. Third-party notices and trademark statements
are in `NOTICE`.
