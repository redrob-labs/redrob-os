# Redrob OS branding (bootc image)

Everything user-visible that says who made this OS lives in the `image/branding/` overlay.
The Containerfile `COPY`s that directory onto `/`, so every file below is already at its
final absolute path. Nothing here needs a build; the PNGs are committed.

## Colour tokens

| Token | Value | Used for |
|---|---|---|
| Redrob red | `#E5322D` (`229,50,45`) | mark, accent colour, selection/highlight, Plymouth arc |
| Background | `#121212` | wallpaper gradient start, Plymouth background, Anaconda sidebar/topbar |
| Background (warm) | `#2A0D0C` | wallpaper gradient end |
| Foreground | `#FCFCFC` | mark inner ring, text on red |

The mark is ours: a red disc (r 0.50) with a white ring (r 0.33–0.21) and a red centre.
It is defined once, in code, in `tools/branding/make_assets.py` (`MARK`), and written by
hand as SVG in the same file. No Fedora, KDE, Universal Blue or Home Assistant geometry
is used anywhere.

## Regenerate

```sh
python3 -m venv ~/.local/venv-pil && ~/.local/venv-pil/bin/pip install pillow
~/.local/venv-pil/bin/python tools/branding/make_assets.py   # writes into image/branding/
```

The script is idempotent and only depends on Pillow. Total overlay size is ~0.6 MB.
Hand-written files (os-release, `.plymouth`/`.script`, SDDM/LnF configs, colour scheme,
Anaconda CSS) are not touched by the script.

## Asset list

| Path (absolute, in the image) | Source | Consumer |
|---|---|---|
| `/usr/lib/os-release` | hand-written | everything (`NAME="Redrob OS"`, `ID=redrob-os`, `ID_LIKE=fedora`, `LOGO=redrob-os`) |
| `/usr/share/icons/hicolor/{16..256}x*/apps/redrob-os.png`, `scalable/apps/redrob-os.svg` | generated | `LOGO=` in os-release → KInfoCenter, Discover, GRUB/boot menus that use the distro icon |
| `/usr/share/pixmaps/system-logo-white.png` (252×252), `redrob-os.png` | generated | Plymouth `spinner` watermark fallback, Anaconda, kcm-about-distro `LogoPath` |
| `/usr/share/plymouth/themes/redrob/{redrob.plymouth,redrob.script,logo.png,throbber-0000..0023.png,box,entry,bullet,lock}.png` | script theme hand-written, images generated | Plymouth boot splash + LUKS prompt |
| `/etc/plymouth/plymouthd.conf` | hand-written (`Theme=redrob`) | plymouthd |
| `/usr/share/wallpapers/Redrob/{metadata.json,contents/images/{1920x1080,2560x1440,3840x2160}.png,contents/screenshot.png}` | generated | Plasma wallpaper package (`Plasma/Wallpaper`, accent `#E5322D` via `X-KDE-PlasmaImageWallpaper-AccentColor`) |
| `/etc/sddm.conf.d/redrob.conf` | hand-written (`[Theme] Current=breeze`) | SDDM; `/etc/sddm.conf.d` overrides `/usr/lib/sddm/sddm.conf.d/kde_settings.conf` from Fedora's kde-settings |
| `/usr/share/sddm/themes/breeze/theme.conf.user` | hand-written | Breeze SDDM theme background = Redrob 1080p wallpaper |
| `/usr/share/plasma/look-and-feel/ai.redrob.desktop/{metadata.json,contents/defaults,contents/colors,contents/previews/*}` | hand-written + generated previews | Plasma global theme "Redrob": Breeze widgets/icons/cursors, colour scheme Redrob, `AccentColor=229,50,45`, wallpaper Redrob |
| `/usr/share/color-schemes/Redrob.colors` | derived from KDE Breeze Dark | Plasma colour scheme; also copied into the LnF package as `contents/colors` |
| `/usr/share/anaconda/pixmaps/{sidebar-logo.png 150×69, sidebar-bg.png 406×767, topbar-bg.png 1040×132, redrob-os.css}` | generated + hand-written CSS | Anaconda GTK UI (sizes match fedora-logos so the stock layout holds) |
| `/etc/xdg/kcm-about-distrorc` | hand-written | System Settings → About this System: `Name`, `LogoPath`, `Website`, `ShowBuild` |

### Colour scheme licence

`Redrob.colors` keeps every KDE `SPDX-FileCopyrightText` line from `BreezeDark.colors`
and stays `LGPL-2.0-or-later`; our line is added below theirs. Only `DecorationFocus`,
`DecorationHover`, `ForegroundActive`, `ForegroundLink`, `BackgroundAlternate` and the
`Colors:Selection` `BackgroundNormal` were changed (Breeze blue → Redrob red); the `Name`
translations were dropped so the scheme shows as "Redrob" in every locale.

## Notes for the other agents

### image agent (Containerfile)

- `/usr/lib/os-release` is the file bootc/ostree ships; `/etc/os-release` is a symlink to
  it in the Fedora base, so do not COPY an `/etc/os-release`. If CI wants to stamp the
  build: `RUN sed -i "s/^BUILD_ID=.*/BUILD_ID=${GIT_SHA}/" /usr/lib/os-release`
  (`VERSION`/`VERSION_ID` the same way when `BASE_TAG` changes).
- `fedora-logos` must be swapped for `generic-logos`, otherwise `/usr/share/pixmaps/
  system-logo-white.png` and `/usr/share/anaconda/pixmaps/*` are owned by the Fedora RPM
  and our overlay would be overwritten/conflict on the next layer:
  `RUN dnf -y swap fedora-logos generic-logos && dnf clean all` **before** the
  `COPY image/branding/ /` line. `generic-logos` still ships `fedora-logo-sprite.svg`,
  `start-here.svg` and the `charge` Plymouth theme under generic names; those are
  not user-visible once our LnF/Plymouth theme is the default, so leave them.
- **Plymouth lives in the initramfs.** On Fedora bootc the initramfs is prebuilt in the
  base image, so dropping files under `/usr/share/plymouth` and `/etc/plymouth` is not
  enough; the theme has to be baked into a new initramfs in the Containerfile, after the
  branding overlay is copied:

  ```dockerfile
  COPY image/branding/ /
  RUN plymouth-set-default-theme redrob && \
      KVER="$(ls /usr/lib/modules | head -n1)" && \
      dracut --force --no-hostonly --reproducible --zstd \
             --add "plymouth" \
             --kver "$KVER" "/usr/lib/modules/$KVER/initramfs.img" && \
      rm -rf /var/tmp/* /var/cache/*
  ```

  `plymouth-set-default-theme` writes `Theme=redrob` into `/etc/plymouth/plymouthd.conf`
  (the overlay already contains that, so the call is idempotent) and dracut's `plymouth`
  module copies the configured theme plus `/usr/share/pixmaps/system-logo-white.png` into
  the image. Universal Blue does the same (`dracut --force ...` after their plymouth
  changes). `bootc container lint` must still pass afterwards — the only writable paths
  touched are `/usr/lib/modules/*/initramfs.img` and `/etc/plymouth`.
- Plasma default global theme: the installer agent owns `/etc/xdg/kdeglobals`; see below.

### installer agent (Anaconda, kdeglobals)

- Set the default global theme in `/etc/xdg/kdeglobals`:

  ```ini
  [KDE]
  LookAndFeelPackage=ai.redrob.desktop
  ```

  That package pulls in colour scheme, accent colour, wallpaper, icons and cursors on
  first login; nothing else about Plasma needs to be set for branding.
- **Anaconda profile.** Anaconda picks its profile by exact `ID=` match on os-release
  (`pyanaconda/core/configuration/profile.py: detect_profile`), not `ID_LIKE`. With
  `ID=redrob-os` the stock `fedora` profile is **not** detected, so the installer would
  fall back to Anaconda's unbranded defaults (blue `#2f4265` sidebar, `efi_dir=default`,
  no Fedora GPG keys). Ship `/etc/anaconda/profile.d/redrob-os.conf` in the installer
  overlay:

  ```ini
  [Profile]
  profile_id = redrob-os
  base_profile = fedora

  [Profile Detection]
  os_id = redrob-os

  [User Interface]
  custom_stylesheet = /usr/share/anaconda/pixmaps/redrob-os.css
  ```

  `base_profile = fedora` inherits the Fedora storage/bootloader/network defaults; the
  CSS line swaps in our pixmaps. The installer ISO (bootc-image-builder `anaconda-iso`)
  runs Anaconda from the image itself, so this file and the pixmaps come from this overlay.
- First-login hooks that show a logo should use `/usr/share/pixmaps/redrob-os.png` or the
  `redrob-os` icon name.

### repo agent

- `NOTICE`/`UPSTREAM.md` should list: KDE Breeze Dark colour scheme (LGPL-2.0-or-later,
  `Redrob.colors` is a derivative); Plymouth `script` example theme (GPL-2.0-or-later;
  `redrob.script` follows its structure for the password dialog, images are ours).

## Rules

- No Fedora, KDE, Universal Blue or Home Assistant names or marks on any product surface:
  boot splash, login, desktop, installer, About dialog, wallpapers, icons. They are only
  allowed in `NOTICE`, `UPSTREAM.md` and `docs/`.
- The mark is never recoloured or re-drawn by hand; change `MARK`/tokens in
  `tools/branding/make_assets.py` and regenerate.
- Anything user-visible that names the OS reads it from `/usr/lib/os-release`; do not
  hard-code "Redrob OS" in scripts when `NAME`/`PRETTY_NAME` will do.
- `generic-logos` (not `fedora-logos`) is the logos RPM in the image; our overlay sits on
  top of it.

## Not verified here

Per the brief nothing was built. The following were checked only against upstream sources
and not on a running image: Plymouth script syntax (modelled on plymouth's `themes/script`
example), SDDM reading `theme.conf.user`, the LnF package applying `AccentColor` on Plasma
6.x in Fedora 44, and the Anaconda CSS selectors for the accent widgets. The first CI
image / ISO is where those get confirmed.
