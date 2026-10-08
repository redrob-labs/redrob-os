# Credential broker (F5)

Status: **phase 1 implemented, 2026-10-08** -- `modules/credential-broker/broker/`
(`redrob-broker`, Rust), unit `deploy/systemd/redrob-broker.service`, config
`deploy/config/broker.toml`, verification in `../verification/credential-broker.md`.
Implements requirement F5 and the egress half of F4.

## What phase 1 ships, and where it deviates from the design below

| Design | Phase 1 |
|---|---|
| socket `/run/redrob/broker.sock` 0660 group `redrob-broker` | same; `RuntimeDirectory=redrob` |
| `POST /v1/call` scoped call | same, plus `/v1/health`, admin routes `/v1/credentials`, `/v1/grants`, `/v1/approvals` (peer uid must be `redrob-agent` or root, from `SO_PEERCRED`) |
| per-task egress proxy on a Unix socket for the sandbox | **not yet** -- the sandbox (Podman + gVisor) does not exist on this host. Instead one forward proxy on `127.0.0.1:3128` for the agent, allow-list from `[egress]`, every request audited. `agent.toml [proxy]` points the agent at it |
| store: SQLite, ChaCha20-Poly1305 under a key derived from `device.key`, reusing `zeroclaw-config` | JSON file `store.json` + `.secret_key` (32 random bytes, 0600, broker user) under `/mnt/data/redrob/broker`. Same cipher, **own 40-line implementation**: `zeroclaw-config` pulls postgres, rusqlite, websockets and the whole agent schema, far too much surface for the one process that holds tokens. The device key stays identity-only |
| approval record signed with the device key | approval = `(task, sha256(task, scope, method, path, sorted query, body))` posted by the admin peer; single use; a different hash does not unlock. Signing is phase 2 |
| `spend` above budget needs approval | `spend` always needs approval (`git.push`); `console.infer` is `mutate` until budget accounting exists |
| OAuth refresh in the broker | not yet; the stored value is sent as-is (`bearer` or `header:<Name>`) |
| audit `/data/audit/broker.jsonl`, daily rotation, 90 days | `/mnt/data/redrob/audit/broker-YYYY-MM-DD.jsonl`; no retention/size cap yet |
| paths `/data/...` | host paths are `/mnt/data/redrob/...`; `/data/...` in manifests is the module view (bind of `/mnt/data/redrob`) |

Phase 2 (not started): route the agent's channel adapters and `google_workspace` through
`/v1/call` so Z1 holds no third-party credential; per-task sandbox socket; OAuth refresh;
approval signing; audit retention.

## Problem

The agent runs model-chosen code. Anything the sandbox can read, a prompt
injection can exfiltrate. So OAuth tokens and API keys must never be inside the
sandbox, and the agent process itself should hold as few as possible.

## Trust zones

| Zone | Process | Holds secrets | Network |
|---|---|---|---|
| Z0 broker | `redrob-broker` (host, systemd, user `redrob-broker`) | yes: all OAuth tokens, API keys, device key | outbound to allow-listed vendor hosts only |
| Z1 agent | `redrob-agent` (host, systemd, user `redrob-agent`) | channel bot tokens only (phase 1, see below) | outbound to Console + channel endpoints |
| Z2 sandbox | tool containers (Podman + gVisor) | none | `none` by default; per-task allow-list via broker proxy |

Z2 reaches the outside world only through the broker. Z1 reaches the broker over a
Unix socket. The broker never returns a raw credential to Z1 or Z2.

## Interface

Socket: `/run/redrob/broker.sock` (mode 0660, group `redrob-broker`; Z1 is in that
group, Z2 containers get a bind-mounted per-task socket instead).

Protocol: HTTP/1.1 over the Unix socket, JSON bodies. Two surfaces:

1. **Scoped call** — `POST /v1/call`
   ```json
   {"task": "t-01J…", "scope": "google.gmail.read", "method": "GET",
    "path": "/gmail/v1/users/me/messages", "query": {"q": "is:unread"}, "body": null}
   ```
   The broker maps `scope` to a vendor base URL and a credential, checks the task's
   grant, injects `Authorization`, forwards, and returns status + body. Paths are
   matched against the scope's allowed path patterns; anything else is refused.

2. **Egress proxy** — `CONNECT`-less forward proxy on a per-task socket
   `/run/redrob/tasks/<task>/proxy.sock`, bind-mounted into the sandbox as the only
   route out. The sandbox sets `HTTPS_PROXY=unix:///run/redrob/proxy.sock`
   (Podman passes the socket; `curl`, `pip`, `git` honour the env). The proxy allows
   only the task's domain allow-list, logs every request, and never injects
   credentials — this path is for public fetches (package indexes, docs, git clone
   over https of public repos).

Scopes (initial):

| Scope | Vendor | Credential | Risk class |
|---|---|---|---|
| `slack.read`, `slack.write` | Slack Web API | bot token | write = `external-send` |
| `discord.read`, `discord.write` | Discord REST | bot token | write = `external-send` |
| `google.gmail.read`, `google.gmail.send` | Gmail API | user OAuth (refresh token) | send = `external-send` |
| `google.calendar.read`, `google.calendar.write` | Calendar API | user OAuth | write = `mutate` |
| `google.drive.read`, `google.drive.write` | Drive API | user OAuth | write = `mutate` |
| `git.push` | GitHub/GitLab | deploy key or PAT | always `approval` |
| `console.infer` | Redrob Console | Console API key | metered = `spend` |

## Approval gate

Risk classes map to the gate in `[risk_profiles]`:

| Class | Default |
|---|---|
| `read` | auto |
| `mutate` | auto within the task's granted scopes, logged |
| `external-send` (message, mail, push) | user approval per action, 10-minute batch window per task |
| `delete`, `spend` above budget | user approval per action, no batching |

Approval requests travel over the agent's existing approval path (channel reply or
dashboard). The broker holds the call until the approval record (signed by the
agent with the device key, carrying the exact request hash) arrives. A hash
mismatch is refused; approvals are not reusable.

## Storage

- Store: `/data/credentials/` — SQLite, every secret column encrypted with
  ChaCha20-Poly1305 under a key derived from the device key (`/data/identity/device.key`,
  generated at first boot, F1). The agent's own secrets store (`agent/`,
  `crates/zeroclaw-config/src/secrets.rs`) uses the same algorithm; the broker reuses
  that crate rather than adding a second implementation.
- Grants: `(task, scope, expires_at, granted_by)`. A task's grants die with the task.
- OAuth refresh is the broker's job; access tokens never leave it.

## Audit log

`/data/audit/broker.jsonl`, append-only, one line per call:

```json
{"ts":"2026-10-08T05:40:12Z","task":"t-01J…","scope":"google.gmail.read",
 "method":"GET","host":"gmail.googleapis.com","path":"/gmail/v1/users/me/messages",
 "status":200,"bytes_in":4213,"bytes_out":0,"decision":"auto","approval":null}
```

Egress-proxy lines have `scope: "egress"` and the destination host. The dashboard
(display module, kiosk) renders this file; the user can also copy it off the
device. Rotation: daily files, 90 days kept, size cap enforced before OTA.

## Mapping onto the agent runtime (`agent/`)

Phase 1 (this stage, L0): channel adapters (`crates/zeroclaw-channels`) keep their
bot tokens in the agent's encrypted secrets store. Tools that need user OAuth
(`src/tools/google_workspace.rs`) are the first to be routed through `POST /v1/call`.
Sandbox egress is `network = "none"` in `[runtime.docker]`.

Phase 2: a `broker` provider shim so channel adapters call Slack/Discord via the
broker too, moving bot tokens out of Z1. Then Z1 holds no third-party credential.

Non-goals: the broker is not a general secrets manager for user code, and it does
not do MITM TLS — public egress is domain allow-list only.
