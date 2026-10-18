# image/installer — installer and first-run overlay

Owned by the installer agent. Everything under `etc/` and `usr/` is copied onto
`/` of the Redrob OS image by `image/Containerfile`; the two `*.toml` files are
blueprints consumed by bootc-image-builder / image-builder in CI, not by the image.

| Path | Purpose |
|------|---------|
| `config.toml` | `--type anaconda-iso` blueprint (x86_64 installer ISO): interactive kickstart, ISO label |
| `config-rpi5.toml` | `--type raw` blueprint (aarch64 / Raspberry Pi 5 disk image) |
| `etc/anaconda/profile.d/redrob-os.conf` | Anaconda profile keyed to `ID=redrob-os` (effective with the `bootc-installer` ISO type; inert with legacy `anaconda-iso`) |
| `usr/lib/systemd/system-preset/85-redrob-initial-setup.preset` | enables the first-boot wizard for raw images (needs `initial-setup initial-setup-gui` in the image) |
| `etc/xdg/autostart/redrob-welcome.desktop`, `usr/libexec/redrob/welcome` | first-login welcome dialog (kdialog): connect phone / install apps / later |
| `etc/xdg/{kdeglobals,kwinrc,baloofilerc,kscreenlockerrc,powerdevilrc}` | Plasma *behaviour* defaults (double-click, Nanum fonts, Breeze buttons on the right, indexing on, 10-min lock, lid = suspend). Look-and-feel is the branding overlay's |

Not shipped on purpose: `krunnerrc` (Meta opens the launcher by default) and
`plasma-org.kde.plasma.desktop-appletsrc` (the base image's default is already a
bottom taskbar with the Kickoff start menu).

Image-agent requirements this overlay relies on:

- `initial-setup` and `initial-setup-gui` installed (raw image first-boot wizard).
- `kdialog` and `libnotify` (`notify-send`) present — both are in the Kinoite base.
- Redrob Launcher on `$PATH` as `redrob-launcher`; `redrob-pairing` from the agent overlay.
- `/usr/libexec/redrob/welcome` must keep its executable bit (tracked as 100755).

User docs: `docs/install.md`, `docs/install.ko.md`, `docs/first-run.md`.
