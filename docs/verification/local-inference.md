# local-inference: the router model served offline (L0 host)

Date: 2026-10-09. This records an end-to-end run of the real supervisor driving
the real llama.cpp server against a real GGUF, on the L0 build host (not QEMU —
the module needs no device hardware, only a CPU). It proves the agent's offline
fallback provider actually answers with no network.

## Setup

| Piece | What |
|---|---|
| server | llama.cpp `llama-server`, tag `b4406`, CPU backend, built from source |
| model | `qwen2.5-0.5b-instruct-q4_k_m.gguf` (Qwen2.5-0.5B-Instruct, Apache-2.0), 491,400,032 bytes |
| supervisor | `modules/local-inference/supervisor/target/release/redrob-local-inference` |

The device ships a ~1B class router; 0.5B is used here only to keep the host
run fast. The model is a deployment choice dropped into `models_dir`, not baked
into the image, so the exact file does not change the contract under test.

Launched exactly as the unit does, pointed at a host models dir:

```
redrob-local-inference --models-dir <dir> --server-bin <llama-server> \
  --listen-host 127.0.0.1 --listen-port 8081
```

## Results

**Model discovery + launch + health.** The supervisor logged model selection
and server start, and `/health` returned 200 one second after launch:

```
INFO local-inference supervisor up models_dir=.../li/models bind=127.0.0.1:8081 alias=router
INFO starting router server bin=.../llama-server model=.../qwen2.5-0.5b-instruct-q4_k_m.gguf
HEALTH_OK after 1s
```

**API + alias.** `GET /v1/models` → `['router']` — the alias matches the
agent's `llamacpp.router` provider `model`.

**Intent classification, offline.** `POST /v1/chat/completions` with a router
system prompt, for "rename every .txt in ~/notes to .md":

```json
{"intent": "code", "tool": "rename", "delegate_to_cloud": false}
```

(usage: prompt 80, completion 20 tokens.) A correct, well-formed routing
decision — a local file-edit task kept on-device, not delegated to the cloud.

**Weights are local, not proxied.** "Answer in one word: capital of France?" →
`Paris`. The answer comes from the local GGUF.

## Security: loopback only, zero egress

```
# ss -ltn : the server's only socket
LISTEN 127.0.0.1:8081   0.0.0.0:*   users:(("llama-server",pid=…))
# established OUTBOUND connections for that pid
established connections: 0
```

The server binds **loopback only** (not `0.0.0.0`) and holds **no** outbound
connection — it reaches nothing off-box. `Config::validate` enforces the bind:
`0.0.0.0` and `192.168.x` are rejected at load (unit test
`rejects_non_loopback_bind`). The unit adds `IPAddressDeny=any` on top.

A fully network-isolated namespace run (`unshare -rn`) was **not possible** on
this host — rootless network namespaces are denied (`unshare: Operation not
permitted`). The zero-egress socket evidence above and the local "Paris" answer
stand in for it; the full agent-in-the-loop fallback (console down → agent
switches providers) is a device/L1 integration test, not exercised here.

## Clean shutdown

On SIGTERM the supervisor terminated its child and exited:

```
INFO stop requested; terminating router server
# llama-server procs remaining: 0
```

## Config wiring (static)

`deploy/config/agent.toml`: `model_provider = "custom.console"` (default),
`fallback = ["llamacpp.router"]`, `[providers.models.llamacpp.router]` →
`http://127.0.0.1:8081`, `model = "router"`. The port and alias match the
supervisor defaults (`listen_port = 8081`, `model_alias = "router"`).

## Gates

| Gate | Result |
|---|---|
| `cargo test --locked` (supervisor) | 18 passed |
| `cargo clippy --all-targets` | 0 warnings |
| `cargo fmt --check` | clean |
| `scripts/verify-units.sh` | 10 units verified (incl. `redrob-local-inference`) |
| `scripts/lint-modules.py` | manifests valid |
| `shellcheck` (scripts) | clean |

## Not verified here

- The agent actually failing over from console to the local router (needs the
  agent binary + a paired key + a blocked console); device/L1 test.
- `remote-gpu` backend; vision/Hailo path.
- The buildroot `llama-cpp` cross-build into the image (host-built server used
  for this run); confirmed in the Stage 8 full image rebuild.
