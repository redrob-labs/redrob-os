# Stage 8: dev4 image measurement and final verification (QEMU TCG)

Date: 2026-10-09. Image: `redrob_generic-x86-64-0.1.dev4` — the first full build
carrying the local-inference module and the `llama-cpp` package. Harness:
`scripts/ota-harness/t11_measure.py` (serial-console boot, same VM recipe as
`ota.md`), 2 GB guest.

## local-inference in a real image (the Stage 7 deliverable, boot-verified)

| Check | Result |
|---|---|
| `redrob-local-inference.service` active | **yes** |
| supervisor logs model dir + bind + alias | `models_dir=/mnt/data/redrob/models bind=127.0.0.1:8081 alias=router` |
| idle state: no model provisioned → no `llama-server`, port 8081 closed | **yes** (correct — the model is a separate provision) |
| `llama-server` + supervisor installed | `/usr/bin/llama-server`, `/usr/bin/redrob-local-inference` |
| extra llama CLI tools trimmed from the image | only `llama-server` remains |
| `redrob-infer` user exists | **yes** |
| config installed | `/etc/redrob/local-inference.toml` (127.0.0.1:8081, alias router) |

So the module ships, comes up, and sits in its designed idle state (waiting for
a model) with no running server and no memory cost until a GGUF is provisioned.

## Measurements

| Metric | Target | Measured |
|---|---|---|
| idle memory used | ≤ 1024 MiB | **134 MiB** / 1966 total |
| boot → agent ready (active-enter, monotonic) | — | **28.1 s** (TCG; real hardware is far faster) |
| gateway listening | — | `127.0.0.1:42617` LISTEN |

Idle memory is 134 MiB because the router model is not loaded on a bare image.
With a ~1B Q4 router provisioned and loaded, add roughly 1–1.5 GiB while it
serves; the unit caps that at `MemoryMax=2G`.

## Unit states at settle

| Unit | State | Note |
|---|---|---|
| `redrob-agent` | active | gateway up |
| `redrob-local-inference` | active | waiting for model (idle) |
| `redrob-pairing.timer` | active | fires the pairing banner |
| `redrob-firstboot` | success | `ExecMainStatus=0` |
| `redrob-display-mode` | skipped | headless default (no kiosk mode file) |
| `redrob-broker` | **activating (auto-restart, 8 restarts)** | see finding B1 |
| `redrob-usb-broker` | **activating (auto-restart)** | see finding B2 |
| `haos-swapfile`, `systemd-growfs@mnt-data` | failed | QEMU-only (no swap device / fixed-size disk); not product units |

`systemd is-system-running` reports **degraded** — from the two QEMU-only units
above, plus the two broker findings below.

## Findings: brokers crash-loop on a fresh image (RESOLVED in dev5)

This is the first full-image boot test to exercise the broker daemons (they
were previously verified only on the L0 host via `scripts/l0-broker.sh`). Both
failed at first boot on dev4. Neither was a local-inference regression — these
units were not touched in Stage 7/8. **Both are fixed in dev5** (see the
resolution at the end of this section).

**B1 — credential-broker: cannot create its key.**

```
Error: create key /mnt/data/redrob/broker/.secret_key
Caused by: Permission denied (os error 13)
redrob-broker.service: Main process exited, code=exited, status=1/FAILURE  (×8)
```

The broker runs as `redrob-broker` and writes to `/mnt/data/redrob/broker`.
firstboot `chown redrob-broker:redrob-broker` + `chmod 0700` that directory, but
at runtime the broker still cannot write it — the ownership firstboot intends is
not taking effect against the pre-built data-partition tree. Likely fix:
firstboot's `chown ... || true` is masking a failure, or the `redrob-data`
partition tree ships the dir root-owned; the broker should also create/own its
own state dir (e.g. a `StateDirectory=`/`ExecStartPre`) rather than depending on
firstboot's chown.

**B2 — usb-broker: namespace setup fails on a missing bind source.**

```
redrob-usb-broker.service: Failed to set up mount namespacing:
    /mnt/data/redrob/usb: No such file or directory
Failed at step NAMESPACE spawning /usr/bin/redrob-usb-broker: status=226/NAMESPACE
```

The unit binds `/mnt/data/redrob/usb`, which firstboot creates — but the unit
starts before that path exists, so the mount namespace cannot be built and the
process never execs. Likely fix: order it `After=redrob-firstboot.service`
(as `redrob-broker` already is) and/or create the dir with a `RuntimeDirectory`
/ `ExecStartPre=mkdir -p`.

network-online.target is active in the guest, so the brokers are not blocked on
the network — the failures above are the whole cause.

### Resolution (dev5)

Root cause of both: firstboot ran `DefaultDependencies=no` — too early for a
reliable `chown`-by-name — and the `redrob-data` partition bakes
`broker`/`audit`/`models` with the build host's orphan uid (1000), so
firstboot's `mkdir -p` no-op'd over the existing dirs and the `chown ... || true`
masked whatever failed; `/mnt/data/redrob/usb` was never baked, so the usb
broker's `ReadWritePaths=/mnt/data/redrob/usb` had no source and the mount
namespace could not be built.

Fix: a dedicated oneshot `redrob-state-dirs.service` runs at multi-user time
(users resolvable, `/mnt/data` mounted, no sandbox), creates
`broker`/`audit`/`usb` and `chown`s `broker`/`audit` to `redrob-broker` every
boot — no `|| true`, so a failure blocks the brokers loudly instead of leaving a
crash loop. Both brokers now `Requires=`/`After=redrob-state-dirs.service`.
firstboot no longer provisions those dirs.

Re-verified on a clean dev5 boot:

```
redrob-state-dirs.service   Result=success ExecMainStatus=0 (active)
/mnt/data/redrob/broker     drwx------ redrob-broker redrob-broker
/mnt/data/redrob/audit      drwxr-x--- redrob-broker redrob-broker
/mnt/data/redrob/usb        drwxr-xr-x root root  (created)
redrob-broker.service       active
redrob-usb-broker.service   active
```

## Not possible on this machine (consolidated)

- **Agent-in-the-loop fallback** (console down → agent switches to the local
  router): needs the paired agent + a reachable-then-blocked console; device/L1.
- **Sandbox red-team** (Podman + gVisor escape attempts): needs root + user
  namespaces — unavailable here (`docs/dev-machine.md`).
- **KVM / L2+** and **real device (L3/L4)** boots: no `/dev/kvm`; TCG only.
- **Real connections** (Slack/Google/GitHub through the broker; a real console
  key): not exercised; L1+ with credentials.
- **aarch64 image** (`rpi5_64`): same package set, cross-built agent; not built.
