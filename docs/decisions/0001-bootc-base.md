# ADR 0001: Build Redrob OS as a bootc image on Fedora Kinoite

Date: 2026-10-18. Status: accepted (decision by Janghoon Lee).

## Context

Redrob OS v1 was a Buildroot image derived from Home Assistant Operating System: a
read-only erofs rootfs, RAUC A/B slots, our own kernel configuration, our own packages for
the agent, the brokers, llama.cpp and a KMS kiosk, and a QEMU harness to prove OTA and
power-cut behaviour. It worked, and it cost us in three ways that did not go away:

- **Hand maintenance.** Every package in the image was ours to choose, build, patch and
  re-pin: kernel, toolchain, Mesa, fonts, container engine. Security updates meant
  rebuilding the world. The upstream subtree sat on a `dev` branch commit because the
  x86 target we needed was only coherent there, and every sync was a conflict session.
- **The 16 K page issue.** The Raspberry Pi 5 kernel defaults to 16 KiB pages and gVisor
  refuses to start on arm64 with anything but 4 KiB pages (`docs/open-questions.md` Q2).
  We had to carry a kernel choice (`kernel=kernel8`) and build every arm64 binary with
  that assumption, which is exactly the kind of platform decision we do not want to own.
- **No desktop.** The product direction is "much easier to use than Linux Mint". A kiosk
  dashboard on a dumb buffer is not a desktop; adding one (Wayland compositor, Mesa,
  Qt, an app store, an input method) to Buildroot is a second distribution's worth of
  work.

Meanwhile the industry pattern for exactly this problem exists: a **bootable container**
(bootc). One Containerfile describes the OS; CI builds it into an OCI image, signs it and
publishes it to a registry; devices fetch, stage and switch to new images atomically and
roll back on a failed boot. Fedora Atomic desktops, Universal Blue (Bluefin, Aurora,
Bazzite) and comparable immutable systems ship this way today.

## Decision

Redrob OS is a bootc image **built `FROM quay.io/fedora-ostree-desktops/kinoite:<release>`**
(Fedora Kinoite: Fedora with the KDE Plasma 6 desktop, ostree-based, bootc-enabled).

- The base tag is pinned in one place, `image/base.env`, and bumped by Renovate.
- Our layer is one `image/Containerfile` plus overlay directories copied onto `/`:
  base files, the agent (binary, units, sandbox config, pairing), branding, installer
  config. Downloaded artifacts are pinned by version and hash in `image/pins.env`.
- Output: `ghcr.io/redrob-labs/redrob-os:stable` (+ `:<release>`, `:<git sha>`), signed
  with cosign (keyless, GitHub OIDC); devices verify the signature.
- Updates: `bootc-fetch-apply-updates.timer` daily, staged, applied at reboot; greenboot
  health checks roll back a deployment that fails them; Flatpak apps auto-update.
- Installer: an Anaconda ISO built by bootc-image-builder on release tags (disk
  selection, dual boot, LUKS, user creation); a raw disk image for aarch64 (Pi 5).
- Users get apps from Flathub via Discover; Redrob Launcher is baked in and installs the
  Redrob apps per user.
- The agentic part is unchanged in substance: `agent/` compiled in CI, installed as
  `/usr/bin/redrob-agent`, tasks sandboxed in podman + gVisor, channels, first-boot
  pairing QR, relay remote access, local inference as a podman quadlet.

## Consequences

What we gain:

- Maintenance collapses to merging Renovate pull requests and one `BASE_TAG` edit a year
  (`docs/maintenance.md`). Kernel, desktop, container engine, fonts, input method and
  their security updates are the base distribution's job.
- A real desktop (taskbar, start menu, Settings, app store, Korean input) on day one, with
  the installer, dual boot and disk encryption that come with it.
- Atomic updates and rollback without owning RAUC, bootloader scripts or a QEMU harness;
  the image build on every pull request is the test.
- The 16 K page question disappears: the distro's aarch64 kernel uses 4 KiB pages.

What we lose or accept:

- **Image size ~3–4 GB** installed (the Buildroot rootfs was ~256 MB). Fine for PCs,
  laptops and a Pi 5 on a 16 GB+ card; not for small flash.
- **Release cadence is Fedora's.** A release is supported for about 13 months; we must
  move `BASE_TAG` to a newer release at least once a year or stop receiving updates.
- **Raspberry Pi 5 support depends on the Fedora kernel** (upstream mainline support for
  the Pi 5 is still maturing, e.g. for the GPU and some peripherals). The Pi 5 is a
  secondary target until the base kernel carries it fully; x86-64 PCs are primary.
- Fedora's packaging decisions (no proprietary codecs or drivers by default) apply;
  users add them via Flathub/RPM Fusion-style repositories, not via our image.
- Boot and update behaviour is now verified by greenboot and CI rather than by our own
  power-cut harness; the `docs/verification/` records from the Buildroot era are history.

## Alternatives rejected

- **Keep Buildroot and add a custom desktop (labwc or similar).** Would give the smallest
  image, but makes us the maintainer of a compositor, Mesa, Qt/GTK stacks, an app store
  and an input method, on top of the kernel and toolchain we already carried. The exact
  opposite of "as little maintenance as possible".
- **Ubuntu Core / snaps.** Immutable and self-updating, but the desktop is not a
  first-class target, the app ecosystem would be snaps rather than Flathub, and the
  store and signing infrastructure are tied to a single vendor's account model.
- **NixOS.** Reproducible and declarative, but no atomic container-image update story
  comparable to bootc, a steep model for contributors and users, and a desktop
  experience that still needs assembling. Also not the "industry does it this way"
  pattern for consumer devices.
- **A different bootc base (Silverblue/GNOME, Universal Blue images).** Kinoite was
  chosen for its Windows-like Plasma layout; Universal Blue images were a close option
  and remain a reference, but building directly on the Fedora image keeps one fewer
  party between us and the distribution.
