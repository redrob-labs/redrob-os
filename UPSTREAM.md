# Upstream

This repository tracks two upstreams. The exact point each one sits on is pinned in
[`upstream-base.json`](./upstream-base.json); this file explains it. Read it before
touching `agent/` or `image/base.env`. Org-wide fork rules are in
[redrob-labs/.github FORKS.md](https://github.com/redrob-labs/.github/blob/main/FORKS.md).

| What | Upstream | Pinned at | Where the pin lives |
|---|---|---|---|
| `agent/` (git subtree, full history) | [zeroclaw-labs/zeroclaw](https://github.com/zeroclaw-labs/zeroclaw) | `05f9fe95d` = tag `v0.8.5`, vendored in `931684ff5` | `upstream-base.json` |
| Base OS image (OCI reference) | [Fedora Atomic base](https://fedoraproject.org/atomic-desktops/) — `quay.io/fedora-ostree-desktops/base-atomic` | tag `44` (Fedora 44, no desktop) | `image/base.env` (`BASE_IMAGE`, `BASE_TAG`), mirrored in `upstream-base.json` |

Until 2026-10-18 this repository also vendored
[home-assistant/operating-system](https://github.com/home-assistant/operating-system)
as the `os/` subtree (Buildroot). That tree was removed when the image moved to bootc
(`docs/decisions/0001-bootc-base.md`). Code derived from it survives only in the
first-boot and pairing logic under `image/agent/`; the attribution stays in `NOTICE`.

## The base image

The image is built `FROM ${BASE_IMAGE}:${BASE_TAG}` (`image/Containerfile`). The base is
not vendored: nothing of it is in this repository except the reference. It supplies the
kernel, systemd, Flatpak, podman and bootc (the desktop is ours); we add our overlay on top.

How the pin moves:

- **Within a release** (`44` → newer `44` build): the tag is a moving tag, so every CI
  build picks up the base's latest content automatically. Nothing to edit. The exact
  base digest used by a build is recorded in the built image's labels and in the CI run.
- **To the next release** (`44` → `45`): Renovate opens a pull request that edits
  `BASE_TAG` in `image/base.env` (the `regex` manager in the Renovate config under
  `.github/`), and the same pull request updates `base_image.tag` in
  `upstream-base.json`. Merging it is the whole upgrade; the CI build on that PR is the
  test. Expect one such PR about every six months; a release is supported for about
  thirteen months, so skipping one release is fine, skipping two is not.
- Pinning to a digest instead of a tag is deliberately not done: it would turn every
  base security update into a pull request.

## Syncing `agent/`

Sync branches are `sync/upstream-<tag>`, opened as a pull request into `develop`, and
merged as a **merge commit, never a squash** (a squash destroys the merge base and the
next sync replays work already taken).

```sh
git remote add upstream-agent https://github.com/zeroclaw-labs/zeroclaw.git   # once
git fetch upstream-agent <tag>
git subtree pull --prefix=agent upstream-agent <tag>
```

Then update `upstreams.agent` in `upstream-base.json` in the same pull request. Resolve
conflicts by hand; they cluster on rebranding (keep ours, take their surrounding change).
Do not finish a rebrand while resolving a conflict.

## Copyright notices

Upstream notices are never replaced; ours is added below theirs. The root `LICENSE`
keeps `Copyright 2017 Pascal Vizeli` and adds
`Copyright 2026 Janghoon Lee (Redrob) and contributors`. `agent/LICENSE-*` are
upstream's files and are left untouched. Attribution for every vendored or derived
component, and the trademark statements for the base distribution and desktop, are in
[`NOTICE`](./NOTICE).
