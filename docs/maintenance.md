# Maintenance

The whole recurring maintenance load of Redrob OS is listed on this page. If something
is not here, it is not expected of a maintainer.

## 1. Merge Renovate pull requests

Renovate (configured under `.github/`) opens pull requests for:

| What it bumps | File | How often |
|---|---|---|
| Base image tag (`BASE_TAG`) | `image/base.env` + `upstream-base.json` | once per Fedora release (~6 months) |
| Pinned artifacts: gVisor `runsc`, Redrob Launcher, llama.cpp, anything else downloaded at build time | `image/pins.env` | as upstreams release |
| GitHub Actions versions | `.github/workflows/*.yml` | as actions release |
| `agent/` is **not** Renovate's job: it is a `git subtree` synced by hand (see `UPSTREAM.md`) | — | — |

Each pull request runs the full CI build of the image. If CI is green, merge. If it is
red, the pull request itself is the report: read the failing step, fix or close.

Base content *within* a release (kernel, desktop, security updates) needs no pull request
at all: the tag is a moving tag, so the next `build-image` run (a merge to `main` or a
manual dispatch) picks it up.

## 2. Bump the Fedora major once a year

A Fedora release is supported for about 13 months. Renovate opens the `BASE_TAG` bump
when the next release is published; merging it is the upgrade. If Renovate is off or the
PR was closed, do it by hand:

1. Edit `BASE_TAG=` in `image/base.env` (e.g. `44` → `45`) and `base_image.tag` in
   `upstream-base.json`.
2. Open a pull request. CI builds the image and the installer; read the Containerfile
   steps for packages that were renamed or dropped in the new release.
3. Merge. Devices on `:stable` receive the new release as a normal daily update and roll
   back by themselves if it does not boot.

Do not skip two releases: the gap in base updates would exceed the support window.

## 3. Watch greenboot and CI

- **CI** (`.github/workflows/`): the image build on pull requests and on `develop`, the
  signed publish to `ghcr.io/redrob-labs/redrob-os`, and the installer build on release
  tags. A red `build-image` run means the base moved under us; the log says where.
- **greenboot** on devices: after each update the health checks run at boot; a failing
  check makes the device roll back to the previous image and record it. Users see
  "rolled back" in Settings → Updates. If several devices report the same rollback,
  the fix is a new image, never a device-side intervention.

## Not maintenance

Things that look like chores but are not ours:

- Kernel, desktop, container engine, fonts, input method, Flatpak runtimes: the base
  distribution ships them; we only consume the tag.
- User apps from Flathub: they update themselves via the user-level Flatpak timer.
- Redrob apps (Browser, Office, Canvas, Code): Redrob Launcher updates them in the
  user's home from their own repositories' releases.
- Building images locally: nothing is built outside CI. The one build is the pull
  request's CI run.

## Release

Tag `develop` as `vX.Y.Z`. The release workflow builds the image at that commit, signs
it, publishes it with the `:stable` tag and attaches the installer ISO and the Pi 5 raw
image to the GitHub release. There is no manual step after the tag.
