# L0: agent on the dev host

`scripts/l0-agent.sh {build|init|validate|start|stop|status|logs}` runs the agent
binary (`agent/target/release/zeroclaw`, built with `--features channel-slack`)
directly on the x86 host. State root: `~/.local/state/redrob-agent`.

Measured 2026-10-08, x86_64 release build (48 MB binary, 7m44s build on 32 cores):

| Item | Result |
|---|---|
| `doctor` | config loads; `custom.console` and `llamacpp.router` providers valid |
| `daemon` | starts, `GET /health` 200, pairing required (one-time code printed) |
| idle RSS | 39 MB |
| cron | `cron add-every` / `list` / `remove` persist in `data/cron/jobs.db` |
| secrets | encrypted store, key at `<config_dir>/.secret_key` (ChaCha20-Poly1305) |
| channels | Discord, email, Telegram, webhook, ACP, filesystem in the default bundle; Slack added by feature |

Not verified here: a real inference turn against Redrob Console (no Console key on
this host — add one with `zeroclaw config set providers.models.custom.console.api_key`
and `zeroclaw agent -a default`), Slack/Discord/Google connections (need app
credentials), and the container sandbox (no Podman on this host, so `init` renders
`[runtime] kind = "native"` with the in-process landlock/bwrap sandbox).

Paths the OS image must provide (see `deploy/systemd/redrob-agent.service`):

| Env | Device path |
|---|---|
| `ZEROCLAW_CONFIG_DIR` | `/data/agent` (config.toml, `.secret_key`) |
| `ZEROCLAW_DATA_DIR` | `/data/agent/data` (sqlite: control plane, cron, memory, sessions, devices) |
| `ZEROCLAW_WORKSPACE` | `/data/agent/workspace` |

Pairing (F1): the gateway's `POST /pair` with `X-Pairing-Code` is the primitive;
the first-boot QR encodes `http://<device>:42617` plus that code.
