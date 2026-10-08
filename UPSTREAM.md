# Upstream

This repository vendors two upstreams as `git subtree` with full history. The exact
point each one sits on is pinned in [`upstream-base.json`](./upstream-base.json); this
file explains it. Read both before touching `os/` or `agent/`. Org-wide fork rules are in
[redrob-labs/.github FORKS.md](https://github.com/redrob-labs/.github/blob/main/FORKS.md).

| Prefix | Upstream | Pinned at | Vendored in |
|---|---|---|---|
| `os/` | [home-assistant/operating-system](https://github.com/home-assistant/operating-system) | `ece5bb5d7` (`dev`, 18.3 + 28 commits) | `aff64f0b5` |
| `agent/` | [zeroclaw-labs/zeroclaw](https://github.com/zeroclaw-labs/zeroclaw) | `05f9fe95d` = tag `v0.8.5` | `931684ff5` |

`os/buildroot` is a submodule of the Home Assistant Buildroot fork, pinned by the
subtree's own `.gitmodules` entry (mapped in the root `.gitmodules`).

## Known deviation: `os/` is pinned to a branch, not a tag

The org rule is to follow upstream **tags**. `agent/` complies (`v0.8.5`). `os/` was
vendored from the `dev` branch 28 commits past `18.3` because the x86 generic image we
needed is only coherent there. **The next `os/` sync must land on a tag** (`18.4` or
later) and update `upstream-base.json` accordingly.

## Syncing

Sync branches are `sync/upstream-<tag>`, opened as a pull request into `develop`, and
merged as a **merge commit, never a squash** (a squash destroys the merge base and the
next sync replays work already taken).

```sh
git fetch upstream-os    <tag>
git subtree pull --prefix=os    upstream-os    <tag>
git fetch upstream-agent <tag>
git subtree pull --prefix=agent upstream-agent <tag>
```

Then update `upstream-base.json` in the same pull request. Resolve conflicts by hand;
they cluster on rebranding (keep ours, take their surrounding change). Do not finish a
rebrand while resolving a conflict.

## Copyright notices

Upstream notices are never replaced; ours is added below theirs. The root `LICENSE`
keeps `Copyright 2017 Pascal Vizeli` and adds
`Copyright 2026 Janghoon Lee (Redrob) and contributors`. `os/LICENSE` and
`agent/LICENSE-*` are upstream's files and are left untouched. Attribution for every
vendored component is in [`NOTICE`](./NOTICE).
