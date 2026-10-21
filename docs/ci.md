# CI and releases

Redrob OS is a bootc image. CI builds the image from `image/Containerfile`, signs it and publishes
it to `ghcr.io/redrob-labs/redrob-os`; installed devices pull it from there. Three workflows under
`.github/workflows/`, kept thin on purpose: one check on a pull request, one build per release.

| Workflow | Trigger | What it does |
|---|---|---|
| `pr.yml` | pull request | one job `check`: shellcheck + `py_compile`, then podman build + `bootc container lint` (never pushed; skipped on promotion PRs to `main`) |
| `pr-policy.yml` | pull request (target) | org rules from `redrob-labs/.github`: open-PR cap, self-review before requesting a maintainer |
| `release.yml` | tag `v*`, manual dry run | the only image build: amd64 (+ experimental arm64) and the task sandbox, tagged `<version>`, `stable`, `44`, `sha-<short>`, `sandbox`, cosign-signed; installer ISO (x86_64) + raw image (aarch64) on a GitHub Release |

Nothing runs on a push to `main`: devices move to a new image when a `v*` tag is released.

## Contract with the Containerfile

CI and `image/Containerfile` agree on these points. Changing one side means changing the other.

- Build context is the **repository root**; the Containerfile is passed with `-f image/Containerfile`.
  So the Containerfile can `COPY agent/ ...`, `COPY image/files/ /`, `COPY image/agent/ /`,
  `COPY image/branding/ /`, `COPY image/installer/ ...` using repo-relative paths.
- The agent is compiled **inside the Containerfile**, in a `FROM rust AS redrob-build` stage that
  builds `agent/` (`cargo build --release --locked` or the `dist` profile) and whose
  `/usr/bin/redrob-agent` the final stage copies in. CI does not compile the agent separately or
  inject a binary. Build it for the stage's own architecture; amd64 and arm64 each run on a native
  runner, so no cross-compilation is needed.
- Every `KEY=value` line of `image/base.env` is passed as `--build-arg KEY=value`
  (comments and blank lines skipped). The Containerfile must declare them: at least
  `ARG BASE_IMAGE` and `ARG BASE_TAG` before `FROM ${BASE_IMAGE}:${BASE_TAG}`.
- CI also passes `--build-arg GIT_SHA=<commit>` always and `--build-arg VERSION=<x.y.z>` on
  releases; the Containerfile may `ARG` them (for `/usr/lib/os-release` `BUILD_ID`/`VERSION_ID`) or
  ignore them.
- `image/pins.env` is read by the Containerfile's own scripts (`image/scripts/`), not by CI. Every
  download is verified against its `*_SHA256`/`*_SHA512` there; a wrong hash fails the build, which
  is the only thing that keeps a Renovate version bump honest (see below).
- The built image must pass `bootc container lint` (run as `podman run --rm <image> bootc container lint`).
- Tags written by CI: `stable`, `<BASE_TAG>` (currently `44`), `sha-<7-char sha>`, and on releases
  `<version>`. Per-arch intermediates `*-amd64`/`*-arm64` are implementation detail.

Install media (`release.yml`) runs `bootc-image-builder` against the pushed `<version>` image with
`--config image/installer/config.toml` (owned by the installer work: users, locale, kickstart
customizations). `--type anaconda-iso` on x86_64 produces `output/bootiso/install.iso`;
`--type raw` on aarch64 produces `output/image/disk.raw`. If bib complains that the image declares
no root filesystem, the image needs `/usr/lib/bootc/install/20-redrob.toml` with
`[install.filesystem.root] type = "xfs"` (or `btrfs`), not a CI change.

## pr.yml

One job, `check`. The image step is the real test: the Rust (agent, compositor) compiles inside
the Containerfile, so there is no separate cargo job.

## release.yml

Trigger: push of a tag `vX.Y.Z` (or `vX.Y.Z-rc.N`). `workflow_dispatch` with a `version` input is a
dry run: it builds and pushes the `<version>` image and builds the media, but publishes no release.

1. `version`: `v1.2.3` -> `1.2.3`; anything not `X.Y.Z[-suffix]` fails. (The repository has no
   version file to cross-check; the tag is the version.)
2. `build` + `manifest`: as above, tagged `<version>`, signed.
3. `iso` (x86_64): pulls the image into root podman storage and runs
   `quay.io/centos-bootc/bootc-image-builder` (pinned by digest in the workflow `env`) with
   `--type anaconda-iso --config image/installer/config.toml` -> `redrob-os-<ver>-linux-x86_64.iso`.
4. `raw` (aarch64, experimental): same with `--type raw` -> `redrob-os-<ver>-linux-aarch64.img.xz`.
5. `publish`: downloads the artifacts, writes `SHA256SUMS`, creates the release
   `Redrob OS v<version>` (`softprops/action-gh-release`), pre-release when the version has a `-`.
   Requires `manifest` and `iso` to have succeeded; `raw` may be missing.

Asset names follow `<name>-<version>-<os>-<arch>`: `redrob-os-1.2.3-linux-x86_64.iso`,
`redrob-os-1.2.3-linux-aarch64.img.xz`, plus `SHA256SUMS`.

### Cutting a release

```sh
git switch main && git pull
git tag -a v1.2.3 -m "Redrob OS 1.2.3"
git push origin refs/tags/v1.2.3:refs/tags/v1.2.3
```

Then watch the `release` run. A failed `raw` job does not block the release; a failed `iso` or
`manifest` does -- fix, delete the tag and the pushed `<version>` image is simply overwritten by the
next attempt. A published release is never overwritten: bump and tag again.

## Secrets and permissions

None to configure. Every job gets `contents: read`; `packages: write` is added where an image is
pushed, `id-token: write` only on the signing jobs, `contents: write` only on the release publish
job. Pushing to GHCR with `GITHUB_TOKEN` needs the `redrob-os` package to allow the repository
(automatic on first push from this repository; check Package settings -> Manage Actions access if
a push returns 403).

The first run of `release.yml` creates the GHCR package; make it **public** in the package
settings so devices can pull without credentials.

## Renovate (`renovate.json` at the repository root)

Weekly (Monday morning KST), never automerged; merge after `pr.yml` is green.

- GitHub Actions: every `uses:` is pinned to a full commit SHA with a `# vX.Y.Z` comment; Renovate
  updates both.
- `image/base.env`: `BASE_TAG` follows the Fedora major of `BASE_IMAGE` (`44` -> `45`), labelled
  `base-image` and grouped on its own. A major bump is a release decision.
- `image/pins.env`: a pin becomes updatable by a comment on the line above its version:

  ```sh
  # renovate: datasource=github-releases depName=redrob-labs/redrob-launcher extractVersion=^v(?<version>.*)$
  REDROB_LAUNCHER_VERSION=0.9.3
  REDROB_LAUNCHER_SHA256=...
  REDROB_LAUNCHER_URL=https://github.com/redrob-labs/redrob-launcher/releases/download/v${REDROB_LAUNCHER_VERSION}/...
  ```

  Renovate bumps only the `*_VERSION` line. The `*_SHA256` must be recomputed by hand on that PR
  (`curl -L <url> | sha256sum`); until then the PR's `image` job fails on the stale hash, which is
  the intended guard.
- `bootc-image-builder` digest in `release.yml` `env` (the `# renovate: datasource=docker ...`
  comment above it).

## What CI does not do

- No per-module cargo tests (`modules/*`), no branch-name or back-merge procedure checks (removed
  with the Buildroot workflows). CI gates are code: build, lint, image.
- No rechunking of the image (`rpm-ostree compose build-chunked-oci`); add it in `release.yml`
  if update download sizes become a problem.
- No qemu cross-builds: arm64 uses GitHub's native arm runners (`ubuntu-24.04-arm`, free for public
  repositories).
