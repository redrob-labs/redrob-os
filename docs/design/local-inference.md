# local-inference: the on-device router model

The agent's default brain is Redrob Console (a remote model). The device must
still do something useful when the network is down, and it should not send
every trivial request to the cloud. `local-inference` serves a small (~1B)
router model on the device so the agent has an **offline fallback** and a
cheap first responder for simple intents.

## What runs

```
agent  --(OpenAI chat_completions)-->  127.0.0.1:8081  (llama.cpp llama-server)
                                              ^
                                   redrob-local-inference (supervisor)
                                              |
                                   /mnt/data/redrob/models/*.gguf
```

- **llama.cpp `llama-server`** (`os/buildroot-external/package/llama-cpp`,
  CPU backend, no libcurl, no OpenMP) exposes an OpenAI-compatible API.
- **`redrob-local-inference`** (`modules/local-inference/supervisor`, Rust) is a
  thin supervisor, not a second inference engine. It picks the model file,
  launches `llama-server` bound to loopback, and relaunches it if it exits.

The supervisor is deliberately small. The routing itself — console first,
local on failure — lives in the agent's provider config, not here.

## Why a supervisor and not a bare `ExecStart`

Two jobs systemd cannot do on its own:

1. **The model is provisioned separately from OTA.** `/mnt/data/redrob/models`
   can be empty at boot and gain a GGUF later. The supervisor polls for it and
   starts serving when it appears, instead of crash-looping while it is absent.
2. **Model selection.** `select_model` (pure, unit-tested) picks, in order:
   an explicit `model` path; `router.gguf`; else the newest `*.gguf`,
   preferring a name containing `router`. A device can hold a plain generation
   model and a router model side by side.

## How the agent reaches it

`deploy/config/agent.toml`:

```toml
[providers.models.custom.console]
uri = "https://console.redrob.ai/api/backend/v1"
model = "auto"
fallback = ["llamacpp.router"]      # <- on failure, use the local router

[providers.models.llamacpp.router]
uri = "http://127.0.0.1:8081"
model = "router"                    # <- matches the supervisor's --alias

[agents.default]
model_provider = "custom.console"   # <- console is the default
```

So the intent / tool-choice / escalation path is: the agent asks console;
when console is unreachable the agent falls back to `llamacpp.router`, which is
this module. The local model classifies the request and either answers it or
signals that it needs the (currently unreachable) cloud — the device degrades
instead of going dark.

## Security: loopback only, zero egress

The router answers raw prompts with **no authentication**, so it must never be
reachable off the device.

- `Config::validate` refuses any `listen_host` that is not a loopback IP
  literal — a non-loopback bind fails at load, it is not a runtime warning.
- `build_argv` always passes the configured loopback `--host`; nothing in the
  command opens a wildcard bind.
- The unit adds `IPAddressAllow=localhost` + `IPAddressDeny=any`, so even a
  compromised server process has no route to the network, plus the usual
  hardening (`ProtectSystem=strict`, empty `CapabilityBoundingSet`,
  `MemoryDenyWriteExecute`, `NoNewPrivileges`). It runs as `redrob-infer`,
  whose only job is to read the GGUF.

## Not in scope here

- **Vision / Hailo.** The AI HAT+ 2 (Hailo-8) is a quantized-CNN vision
  accelerator, not an LLM engine, and its compiler is x86-only. HEF vision
  models are cross-compiled off-device and dropped into `models_dir`; see
  `docs/open-questions.md` Q3. This module's router path is CPU llama.cpp.
- **remote-gpu.** A tailnet GPU box is a declared backend in `module.yaml`;
  wiring it is a later pass. Today the fallback is the local CPU model.
