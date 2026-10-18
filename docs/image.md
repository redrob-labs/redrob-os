# Redrob OS image (bootc)

Redrob OS is one bootable container image: `ghcr.io/redrob-labs/redrob-os`.
`image/Containerfile` describes the whole operating system; CI builds it, signs it and
pushes it; devices pull it with `bootc`, stage it, and roll back if the new deployment
fails its health checks. Nobody maintains packages by hand -- the Fedora Kinoite base
does, and we layer only what makes it Redrob OS.

Tags: `stable` (what devices follow), `44` (the Fedora release the image is built on),
`<git sha>` (every build), `vX.Y.Z` (releases).

## Layer order

The Containerfile applies, in order:

| # | Step | Source | Notes |
|---|------|--------|-------|
| 1 | Base image | `image/base.env` -> `quay.io/fedora-ostree-desktops/kinoite:44` | KDE Plasma 6, bootc, podman, flatpak already inside. |
| 2 | Packages | `image/Containerfile` (one `dnf5` transaction) | `fedora-logos` -> `generic-logos` swap; `naver-nanum-fonts-all`, `ibus-hangul`, `greenboot` + default checks, `distrobox`, `plasma-discover-flatpak`, printing (`cups`, `plasma-print-manager`), scanning (`sane-backends*`), `fwupd`, `tuned-ppd`, `zstd`. `install_weak_deps=False`. |
| 3 | Pinned binaries | `image/scripts/10-pins.sh`, `20-launcher.sh`, `image/pins.env` | gVisor `runsc` -> `/usr/libexec/gvisor` (+`/usr/bin` symlinks, podman runtime `runsc`); Redrob Launcher -> `/usr/bin/redrob-launcher`, desktop entry, icons, autostart. Every download is checksum-verified. |
| 4 | Base-OS overlay | `image/files/` COPYed onto `/` | Update timers, greenboot presets and check, Flathub unit, container signature policy, sysctl, README. |
| 5 | Branding overlay | `image/branding/` | `/usr/lib/os-release`, logos, wallpaper, SDDM/Plymouth/Plasma look. Owned by the branding work; **nothing in `image/files/` writes os-release.** |
| 6 | Agent overlay | `image/agent/` | `redrob-agent` units, sandbox config, pairing. |
| 7 | Installer overlay | `image/installer/` | first-login hooks. |
| 8 | Cleanup | `image/scripts/30-cleanup.sh` | Applies our presets (`systemctl preset` for each unit named in `90-redrob.preset`), masks Fedora Flatpak remotes, empties `/var`, `/opt -> var/opt`, exec bits on greenboot checks, sanity asserts (launcher, runsc, `generic-logos` present, `fedora-logos` gone). |
| 9 | Commit + lint | `ostree container commit`, `bootc container lint` | Lint failure fails the build. |

Later overlays win on path collisions. Put files at their final absolute path inside the
overlay directory (`image/files/etc/sysctl.d/90-redrob.conf` lands at `/etc/sysctl.d/90-redrob.conf`).

`/var` is machine state and must be empty in the image; pre-create directories with
`image/files/usr/lib/tmpfiles.d/redrob.conf` (or `StateDirectory=` in units), never by
shipping them in the image.

## Building locally (once, when you need to)

The build context is the **repository root**, because the Containerfile COPYs
`image/files`, `image/branding`, `image/agent`, `image/installer`.

```sh
set -a; . image/base.env; set +a
podman build \
  -f image/Containerfile \
  --build-arg BASE_IMAGE="$BASE_IMAGE" \
  --build-arg BASE_TAG="$BASE_TAG" \
  --build-arg REDROB_OS_VERSION="dev" \
  --build-arg REDROB_OS_GIT_SHA="$(git rev-parse --short HEAD)" \
  -t localhost/redrob-os:dev .
```

`ARG BASE_IMAGE`/`ARG BASE_TAG` in the Containerfile carry the same defaults as
`image/base.env`; the env file is the source of truth and CI passes it in exactly as
above (`cat image/base.env >> "$GITHUB_ENV"` then `--build-arg`). Cross-building for
aarch64 needs `qemu-user-static` and `--platform linux/arm64`; the scripts pick the
architecture from `TARGETARCH` (podman sets it) or `uname -m`.

The build needs network access for the dnf repositories and the two pinned downloads.
The dnf cache is a `--mount=type=cache`, so repeated local builds are fast.

Run the lint again on a built image if you want to see it outside the build:

```sh
podman run --rm -it localhost/redrob-os:dev bootc container lint
```

## Testing on a running Kinoite / Redrob OS machine

Any Fedora Atomic desktop (or an older Redrob OS) can rebase onto the image:

```sh
sudo bootc switch ghcr.io/redrob-labs/redrob-os:stable
sudo systemctl reboot
```

To try a CI build before it is promoted to `stable`, switch to its sha tag:
`sudo bootc switch ghcr.io/redrob-labs/redrob-os:<git sha>`. Back out with
`sudo bootc rollback` (keeps the previous deployment; greenboot does the same thing
automatically after repeated failed boots). `bootc status` shows booted / staged /
rollback deployments.

A locally built image can be tested by pushing it to a registry you control or with
`sudo bootc switch --transport containers-storage localhost/redrob-os:dev` on the
machine that built it.

Signature policy applies to `bootc switch`/`upgrade` too: the pull is refused until
`/etc/pki/containers/redrob-os.pub` on the device matches the key CI signs with
(see Signing). On a plain Fedora Kinoite, the first `switch` is unverified (Fedora's
default policy accepts anything); from then on the image's own policy is in effect.

## Updates on devices

- **OS**: `bootc-fetch-apply-updates.timer`, daily at 03:00 local (+ up to 2 h random
  delay, `Persistent=true`). Our drop-in changes the service to `bootc upgrade` (stage
  only) instead of upstream's `--apply`, so a desktop is never rebooted under someone.
  The staged image boots on the next reboot; `bootc upgrade --apply` applies now.
- **Health**: greenboot runs `/etc/greenboot/check/required.d/*` on every boot. Red boot ->
  the boot counter increments and grub falls back to the previous deployment after the
  configured number of failures (`/etc/greenboot/greenboot.conf`, default 3). Ours:
  the Fedora default checks (`greenboot-default-health-checks`: required scripts
  present, update platform/DNS reachable, hardware watchdog) plus `20-redrob-base.sh`
  (launcher, runsc, podman, flatpak and the signature policy exist; `bootc status` works;
  `runsc --version` runs).
- **Apps**: Flathub is added once by `redrob-flathub.service` (marker
  `/var/lib/redrob/flathub-added`); Fedora's own Flatpak remote unit is masked.
  `redrob-flatpak-update.timer` (user, daily, `Persistent=true`) runs
  `flatpak update -y` and prunes unused runtimes. Discover shows the same apps.
- **Redrob apps**: Redrob Launcher autostarts at login (`/etc/xdg/autostart/ai.redrob.launcher.desktop`)
  and installs/updates Browser/Office/Canvas/Code into the user's home, as before.

## Pins (`image/pins.env`)

Every artifact the build downloads has `NAME_VERSION`, `NAME_URL` (may use `${ARCH}`,
`x86_64|aarch64`) and `NAME_SHA256`/`NAME_SHA512` (per-arch suffixes
`_X86_64`/`_AARCH64` when the file differs by arch). `image/scripts/lib.sh` resolves
and verifies them; a missing pin or `TODO-hash` fails the build.

Bumping a pin, e.g. gVisor:

```sh
V=20261110.0
for a in x86_64 aarch64; do
  curl -L -o "$KIROCREW_SCRATCH/gvisor.$a.tar.zstd" \
    "https://storage.googleapis.com/gvisor/releases/release/$V/$a/gvisor.tar.zstd"
  curl -L "https://storage.googleapis.com/gvisor/releases/release/$V/$a/gvisor.tar.zstd.sha512"
  sha512sum "$KIROCREW_SCRATCH/gvisor.$a.tar.zstd"   # must match the line above
done
```

then change `GVISOR_VERSION` and both `GVISOR_SHA512_*` in the same commit
(`feat(bootc): bump gvisor to 20261110.0`). gVisor's `latest/` directory is a moving
alias and is not pinnable; always use the dated release path. Redrob Launcher: take the
two `linux-*.tar.gz` lines from the release's `SHA256SUMS.txt`
(`gh release view -R redrob-labs/redrob-launcher --json assets`).

Current pins: gVisor `20261012.0` (`runsc version release-20261012.0`), Redrob Launcher `0.7.0`.

Installed versions are recorded in the image at `/usr/share/redrob/pins-installed.env`.

## Signing

Devices require a cosign signature on `ghcr.io/redrob-labs/redrob-os`
(`/etc/containers/policy.json`, `sigstoreSigned`, `signedIdentity: matchRepository`;
`/etc/containers/registries.d/redrob-os.yaml` turns on sigstore attachments for that
repository). Everything else (Flathub, Distrobox images) keeps the permissive default.

The plan called for keyless signing (GitHub OIDC). **The client side cannot verify that
today**: containers-policy.json's `fulcio` requirement needs `subjectEmail` and GitHub
Actions certificates carry a workflow URI, not an email, so podman/bootc would reject
every keyless signature. This is the same reason Universal Blue signs with a key pair.
So:

1. Done 2026-10-18: a cosign key pair was generated (no password). The private key is the
   repository secret `COSIGN_PRIVATE_KEY`; the public key is committed as
   `image/files/etc/pki/containers/redrob-os.pub`. The same key signs the task sandbox
   image, published in the same package as `ghcr.io/redrob-labs/redrob-os:sandbox`
   (so policy.json needs one repository entry).
2. CI, after push: `cosign sign --yes --key env://COSIGN_PRIVATE_KEY <image>@<digest>`,
   then a keyless signature *in addition* (nice for `cosign verify` by humans; it does not
   satisfy the device policy).
3. Rotating the key = shipping the new `.pub` in an image signed with the **old** key
   first, then switching CI to the new key.

If the public key ever goes missing or mismatches, the policy fails closed: devices refuse
to pull the image rather than accept an unsigned one.

## Owned files

| Path | Purpose |
|------|---------|
| `image/Containerfile` | the build |
| `image/base.env` | base image + tag (the only place) |
| `image/pins.env` | downloaded-artifact pins (other owners append sections) |
| `image/scripts/lib.sh` | arch detection, pin loading, verified download |
| `image/scripts/10-pins.sh` | gVisor install + podman runtime registration |
| `image/scripts/20-launcher.sh` | Redrob Launcher install + autostart |
| `image/scripts/30-cleanup.sh` | presets, Flatpak remote mask, `/var`/`/opt`/`/boot` shape, asserts |
| `image/files/usr/lib/systemd/system-preset/90-redrob.preset` | bootc timer, greenboot, flathub, cups, fwupd-refresh |
| `image/files/usr/lib/systemd/user-preset/90-redrob.preset` | user flatpak update timer |
| `image/files/usr/lib/systemd/system/bootc-fetch-apply-updates.{timer,service}.d/redrob.conf` | daily 03:00, stage-only |
| `image/files/usr/lib/systemd/system/redrob-flathub.service` | one-shot Flathub remote |
| `image/files/usr/lib/systemd/user/redrob-flatpak-update.{service,timer}` | per-user app updates |
| `image/files/usr/lib/tmpfiles.d/redrob.conf` | `/var/lib/redrob`, `/var/opt` |
| `image/files/etc/containers/policy.json`, `registries.d/redrob-os.yaml`, `image/files/etc/pki/containers/redrob-os.pub` | signature policy |
| `image/files/etc/greenboot/check/required.d/20-redrob-base.sh` | boot health check |
| `image/files/etc/sysctl.d/90-redrob.conf` | inotify limits |
| `image/files/usr/share/redrob/README` | what is on the machine and where |

Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
