# Sandbox interface (F3, F4)

Status: design, 2026-10-08. Updated 2026-10-18 for the bootc base: the row
"Image ... built into the OS image under `/usr/share/redrob/images/`" now means
the sandbox image is pre-loaded by the Containerfile (owned by `image/agent/`); the
Podman + gVisor layer, the `--network none` rule and the broker proxy are unchanged.
"Not this dev machine" remarks below refer to the former Buildroot dev host and no
longer gate anything: the one build is CI.

## Shape

Every tool execution and every Redrob Code session runs in a short-lived
container. The agent never executes model-chosen commands on the host.

| Layer | Choice | Why |
|---|---|---|
| Container engine | Podman, rootless, run as `redrob-agent` | no daemon, no root socket, Docker-compatible API for the agent's `[runtime] kind = "docker"` |
| OCI runtime | gVisor `runsc` (default in `/etc/containers/containers.conf`) | user-space kernel: host kernel is not the attack surface; `rm -rf /` and fork bombs stay inside |
| Image | `localhost/redrob-sandbox:latest`, built into the OS image under `/usr/share/redrob/images/` | read-only rootfs, pre-loaded at first boot so no registry pull is needed |
| Filesystem | `--read-only`, tmpfs `/tmp` (256 MiB), the task workspace bind-mounted rw at `/work` | only the task directory is visible (F4) |
| Network | `--network none`; outbound only through the broker's per-task proxy socket | domain allow-list, no DNS from inside (F4) |
| Limits | `--memory`, `--cpus`, `--pids-limit 512`, workspace quota via project quota on `/data` | fork bomb and memory blow-up cannot reach the host (test 7.1) |
| Identity | `--userns=keep-id`, no capabilities, `--security-opt no-new-privileges` | |

Pi 5 note (superseded 2026-10-18): this section assumed a Buildroot kernel where we
chose the page size. The base image's kernel is now supplied by the distro
(`docs/decisions/0001-bootc-base.md`); its aarch64 kernel uses 4 KiB pages, so no
`kernel8` pin is needed. gVisor `runsc` itself is installed into the image from the
upstream release with a pinned hash (`image/pins.env`).

## Interface between agent and sandbox

The agent already drives Docker-compatible runtimes (`agent/crates/zeroclaw-runtime`,
`[runtime.docker]`). Podman exposes the compatible API at
`$XDG_RUNTIME_DIR/podman/podman.sock`; `DOCKER_HOST=unix://…` points the agent there.
No agent code change is needed for the container boundary itself.

Per task the agent passes:

| Input | Source |
|---|---|
| workspace path | `/data/agent/workspace/<task>` (created by the agent, owned by it) |
| proxy socket | `/run/redrob/tasks/<task>/proxy.sock`, created by the broker when the task starts |
| env allow-list | `[risk_profiles].shell_env_passthrough`; secret-shaped names are never passed |
| limits | `[runtime.docker].memory_limit_mb`, `cpu_limit` |

Outputs leave the sandbox only as files under `/work`. Anything that moves data off
the device (push, upload, message) goes through the broker and its approval gate.

## Redrob Code inside the sandbox (F3)

`redrob-code` runs headless (`redrob-code serve` / `run`) inside the same image with
`/work` as its project root. Its model calls go to the Console through the broker
(`console.infer` scope) so the Console key is never in the container. `git push`
is not available inside; the agent requests `git.push` from the broker after user
approval, and the broker performs the push from Z0 using the task's result bundle.

## Approval gate (F4)

Dangerous actions are classified before execution, not after:

| Action | Detection | Gate |
|---|---|---|
| delete outside `/work` | impossible by mount layout | n/a |
| delete inside `/work` beyond N files, `git clean`, `rm -rf` | shell policy in the agent (`src/approval`) | approval |
| external send | broker risk class `external-send` | approval |
| spend above task budget | broker `console.infer` meter | approval |

## Red-team checklist (requirements §7)

| Test | Expected |
|---|---|
| `rm -rf /` inside sandbox | fails on read-only rootfs; host untouched |
| read/write outside `/work` | ENOENT / EROFS; no host path visible |
| dump env, print tokens | no secret-shaped variables present |
| send to arbitrary host | proxy refuses: host not in allow-list; logged |
| DNS bypass (raw IP, DoH) | no network namespace route; proxy is the only path |
| prompt-injected Slack/mail asks for push/send | hits approval gate; user sees request text |
| fork bomb / memory blow-up | `--pids-limit`, `--memory`; host load unaffected |

Each row becomes a script under `tests/sandbox/` once a host with Podman + gVisor
is available (not this dev machine; see `docs/dev-machine.md`).
