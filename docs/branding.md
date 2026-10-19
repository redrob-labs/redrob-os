# Redrob OS branding (bootc image)

Everything user-visible that says who made this OS lives in the `image/branding/` overlay.
The Containerfile `COPY`s that directory onto `/`, so every file below is already at its
final absolute path. Nothing here needs a build; the PNGs are committed.

## Source: the Redrob Group Design System

Redrob OS carries the group brand (group tier, `10-logo.md`: it is not one of the seven
products, so Redrob grounds, the Redrob lockup, no product colour). Nothing is drawn by
hand. `tools/branding/brand/` holds verbatim copies of the design system's shipped files,
pinned by `SHA256SUMS`; `tools/branding/make_assets.py` only resizes **down**, crops,
composes grounds and writes config files from them.

| In `tools/branding/brand/` | From | Used for |
|---|---|---|
| `redrob-logo-primary-white.png` | `Logos/` (gradient symbol, white wordmark) | Plymouth splash, Anaconda sidebar |
| `redrob-logo-primary.png` | `Logos/` | reserved for light grounds (first-login pages) |
| `redrob-logo-primary-solid-mono-white.png` | `Logos/` (flat white) | anything placed on a gradient |
| `redrob-symbol-solid-white.png`, `symbol-white-*.png` | `Symbol/`, `Favicons/` | `system-logo-white.png` (KInfoCenter header, Plymouth fallback) |
| `favicon-{16,32,48,64}.png`, `icon-{192,512}.png`, `favicon.svg` | `Favicons/` (white symbol on a Redrob Blue tile, drawn per size) | the `redrob-os` icon: start button, About, Discover, os-release `LOGO=` |
| `tokens.json` | `@redrob-labs/ui` 1.7.0 `dist/styles/tokens.json` | every colour below |

### Colour tokens (from `tokens.json`)

| Token | Light | Dark | Used for |
|---|---|---|---|
| `blue-6` Redrob Blue (`surface-brand`, `focus-ring`) | `#2B52FF` | `#2B52FF` | accent colour, selection, checked controls, installer progress, favicon tile |
| `blue-7` / `blue-4` (`ink-brand`) | `#1733D5` | `#8AAFFF` | links, active text, hover |
| `redrob-black` (`surface-base` dark) | - | `#0A0B0C` | Plymouth field, wallpaper field, Anaconda sidebar/topbar, login/lock (`Colors:Complementary`) |
| `surface-base` / `surface-raised` / `surface-sunken` | `#FFFFFF` / `#F8F9FB` / `#EFF1F4` | `#0A0B0C` / `#141719` / `#1C1F26` | View / Window+Header+Tooltip / alternate rows, buttons |
| `ink-primary` / `ink-muted` | `#0A0B0C` / `#686E78` | `#F8F9FB` / `#838A97` | text / inactive text |
| `status-danger` / `-warning` / `-success` | `#A31310` / `#AE5100` / `#00864A` | `#FF4B4B` / `#FF9C1B` / `#29E474` | Negative / Neutral / Positive |
| `blue-10` | `#030C34` | | the lit far side of the wallpaper seam |
| `gradient-brand` = `blue-6` -> `accent-teal-3` `#6FF4F0` at 40 deg | | | the seam, the Plymouth loader segment |

Two colour schemes are generated: `Redrob` (light, the default -- the design system's
`surface-base` is white, as is Windows') and `RedrobDark`, selectable in System Settings.
The login greeter and lock screen draw from `Colors:Complementary`, which is the dark set
in both schemes because it sits on the wallpaper.

### Grounds

The wallpaper and the Anaconda sidebar are the design system's **deep** ground
(`85-social.md`): Redrob Black crossed by one diagonal at 40 deg toward the bottom right,
the far side lit `blue-10`, the seam itself `gradient-brand` with one hairline. No logo on
the wallpaper; the lockup appears once per surface (splash, installer sidebar).

### Typography

`30-typography.md`: Pretendard (Latin system face) and Wanted Sans (Korean system face),
both SIL OFL, installed from their upstream release zips by `image/scripts/25-fonts.sh`
(pins in `image/pins.env`). `/etc/fonts/conf.d/50-redrob.conf` makes Pretendard the
default sans and lets it fall through to Wanted Sans for Hangul; `/etc/xdg/kdeglobals`
names Pretendard for every Plasma font role. Nanum Myeongjo (Fedora's `naver-nanum-fonts`)
stays as the Korean serif voice face.

## Regenerate

```sh
python3 -m venv ~/.local/venv-pil && ~/.local/venv-pil/bin/pip install pillow
~/.local/venv-pil/bin/python tools/branding/make_assets.py   # writes into image/branding/
```

The script verifies `tools/branding/brand/SHA256SUMS` first and refuses to enlarge any
artwork. Hand-written files (os-release, `.plymouth`/`.script`, PLM/LnF configs, the
layout script) are not touched by it; colour schemes, Anaconda CSS, wallpaper metadata
and every PNG are. To take a new design-system export: copy the file over, update
`SHA256SUMS`, rerun.

It ends with a **token audit**: every colour literal in the text files of
`image/{branding,installer,files,agent}` (hex, and `r,g,b` triplets in KDE colour files,
`kdeglobals` and LnF `defaults`) must be a value in `tokens.json`, and Plymouth's float
background must be `redrob-black`; anything else fails the run with the file and value.
A hand-written file with an off-token colour (the old `#E5322D` accent, say) is caught
the next time anyone regenerates.

### Surfaces and where their colours come from

| Surface | Source of colour |
|---|---|
| Plasma widgets, windows, panel, notifications, System Settings, Discover | `Redrob` / `RedrobDark` colour schemes (tokens) via `/etc/xdg/kdeglobals` |
| GTK and Flatpak apps | the same scheme, mirrored by KDE's GTK config sync |
| Accent | `AccentColor=43,82,255` (`blue-6`) |
| Desktop, login greeter, lock screen | the deep-ground wallpaper; set in the layout script, `plasmalogin/defaults.conf`, `/etc/xdg/kscreenlockerrc` |
| Login splash (KSplash) | LnF `contents/splash/Splash.qml`: `redrob-black`, `gray-8`, `gradient-brand`; `/etc/xdg/ksplashrc` |
| Boot splash, LUKS prompt | Plymouth `redrob` theme (tokens) |
| Installer (Anaconda) | `redrob-os.css` + pixmaps (tokens) through `images/product.img`, with the token typefaces |
| Terminal | Konsole `Redrob` scheme/profile (dark tokens; ANSI = accent families) |
| Text | Pretendard / Wanted Sans (`font-sans`, `font-sans-kr`) |

Not driven by tokens, by choice or because no token exists: the Breeze icon and cursor
themes (the design system ships no icon theme for a desktop), and NanumGothicCoding as
the fixed-width UI font (`font-mono` names the platform monospace). The GRUB menu is
plain text with 16 VGA colours, so `usr/lib/bootupd/grub2-static/configs.d/40-redrob-colors.cfg`
maps the tokens to the nearest ones (black ground, light-gray text, white on blue
selection); bootupd writes it into `/boot/grub2/grub.cfg` at install time, so systems
installed before it keep the stock colours.

## Asset list

| Path (absolute, in the image) | Source | Consumer |
|---|---|---|
| `/usr/lib/os-release` | hand-written | everything (`NAME="Redrob OS"`, `ID=redrob-os`, `ID_LIKE=fedora`, `LOGO=redrob-os`) |
| `/usr/share/icons/hicolor/{16..256}x*/apps/redrob-os.png`, `scalable/apps/redrob-os.svg` | design-system favicon tiles (64->128 and 512->256 downscaled) | `LOGO=` in os-release → KInfoCenter, Discover, GRUB/boot menus that use the distro icon |
| `/usr/share/pixmaps/system-logo-white.png` (252×252, white symbol on transparent), `redrob-os.png` (tile) | design-system symbol / tile, downscaled | Plymouth `spinner` watermark fallback, Anaconda, kcm-about-distro `LogoPath` |
| `/usr/share/plymouth/themes/redrob/{redrob.plymouth,redrob.script,logo.png,throbber-0000..0023.png,box,entry,bullet,lock}.png` | script theme hand-written; `logo.png` = primary-white lockup at 420 px; loader frames generated | Plymouth boot splash (lockup still, loader bar moves -- `10-logo.md` forbids animating the mark as a loading indicator) + LUKS prompt |
| `/etc/plymouth/plymouthd.conf` | hand-written (`Theme=redrob`) | plymouthd |
| `/usr/share/wallpapers/Redrob/{metadata.json,contents/images/{1920x1080,2560x1440,3840x2160}.png,contents/screenshot.png}` | generated | Plasma wallpaper package (`Plasma/Wallpaper`, the deep ground; accent `#2B52FF` via `X-KDE-PlasmaImageWallpaper-AccentColor`) |
| `/usr/lib/plasmalogin/defaults.conf` | hand-written | Plasma Login Manager (Kinoite 44's display manager; SDDM is not installed) distro defaults: Redrob wallpaper package, clock on. Admins/KCM override in `/etc/plasmalogin.conf` |
| `/etc/xdg/kdeglobals` `[General] ColorScheme=Redrob`, `AccentColor=43,82,255`, Pretendard font roles (installer overlay) | hand-written | colours of the login greeter (runs as `plasmalogin`, no LnF applied) and of every user before the LnF package runs |
| `/usr/share/plasma/look-and-feel/ai.redrob.desktop/{metadata.json,contents/defaults,contents/colors,contents/layouts/org.kde.plasma.desktop-layout.js,contents/previews/*}` | hand-written + generated colours/previews | Plasma global theme "Redrob": Breeze widgets/icons/cursors, colour scheme Redrob, `AccentColor=43,82,255`, wallpaper Redrob, default panel whose start button is the `redrob-os` tile and whose task manager is icons-only |
| `/usr/share/color-schemes/{Redrob,RedrobDark}.colors` | generated from `tokens.json` (Breeze section structure) | Plasma colour schemes; the light one is also the LnF package's `contents/colors` |
| `/usr/share/anaconda/pixmaps/{sidebar-logo.png 150×69 (lockup 130 px wide), sidebar-bg.png 406×767 (deep ground), topbar-bg.png 1040×132, redrob-os.css}` | generated | Anaconda GTK UI, delivered to the installer through `images/product.img` (`tools/installer/brand-iso.sh`, release workflow); sizes match fedora-logos so the stock layout holds |
| `/etc/xdg/kcm-about-distrorc` | hand-written | System Settings → About this System: `Name`, `LogoPath`, `Website`, `ShowBuild` |

### Colour scheme provenance

`Redrob.colors` and `RedrobDark.colors` are generated: every value is a design-system
token, and only the *section structure* (`[Colors:View]`, `[WM]`, the `ColorEffects`
blocks) follows KDE's Breeze schemes. No Breeze colour value survives, so the files carry
our Apache-2.0 header; `UPSTREAM.md` still credits Breeze for the structure.

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
  CSS line swaps in our pixmaps.

  **The installer does not run from our image.** bootc-image-builder `anaconda-iso`
  builds the installer root from Fedora packages (fedora-logos, Fedora os-release), so
  nothing in this overlay is present while Anaconda runs -- the v0.3.0 ISO showed the
  Fedora sidebar and "Welcome to Fedora Linux 44". The release workflow therefore adds
  `images/product.img` to the ISO (`tools/installer/brand-iso.sh`): Anaconda's dracut
  stage overlays it onto the installer root. It carries the four Anaconda pixmaps, the
  profile above, a `conf.d` file naming the stylesheet, our os-release and
  `system-logo-white.png`. Verified on the v0.3.1 ISO branded locally with the same
  script: "Welcome to Redrob OS 44", Redrob sidebar and lockup in Anaconda.
- First-login hooks that show a logo should use `/usr/share/pixmaps/redrob-os.png` or the
  `redrob-os` icon name.

### repo agent

- `NOTICE`/`UPSTREAM.md` should list: KDE Breeze colour schemes (LGPL-2.0-or-later;
  `Redrob*.colors` follow their section structure, values are ours); Pretendard and
  Wanted Sans (SIL OFL 1.1, installed from upstream releases); Plymouth `script` example theme (GPL-2.0-or-later;
  `redrob.script` follows its structure for the password dialog, images are ours).

## Rules

- No Fedora, KDE, Universal Blue or Home Assistant names or marks on any product surface:
  boot splash, login, desktop, installer, About dialog, wallpapers, icons. They are only
  allowed in `NOTICE`, `UPSTREAM.md` and `docs/`.
- The mark is never redrawn, recoloured, stretched or enlarged (`10-logo.md`). Only the
  design system's exports in `tools/branding/brand/` are used, and only scaled down. A
  new colour is a new `tokens.json`, not an edit to `make_assets.py`.
- Blue acts: anything that is a control, a link, focus or selection is Redrob Blue. No
  second accent anywhere on the OS.
- Anything user-visible that names the OS reads it from `/usr/lib/os-release`; do not
  hard-code "Redrob OS" in scripts when `NAME`/`PRETTY_NAME` will do.
- `generic-logos` (not `fedora-logos`) is the logos RPM in the image; our overlay sits on
  top of it.

## Not verified here

Per the brief nothing was built. The following were checked only against upstream sources
and not on a running image: Plymouth script syntax (modelled on plymouth's `themes/script`
example), the LnF package applying `AccentColor` on Plasma 6.x in Fedora 44, and the
Anaconda CSS selectors for the accent widgets. The first CI image / ISO is where those get
confirmed. Confirmed on the v0.2.6 QEMU install: Plymouth theme loads, Anaconda shows
"Redrob OS 44"; the login screen was still stock because Kinoite 44 runs Plasma Login
Manager, not SDDM (fixed in v0.3.0 by `/usr/lib/plasmalogin/defaults.conf`).
