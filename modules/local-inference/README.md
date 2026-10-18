# local-inference module

Serves a small (~1B) "router" model on loopback so the agent has an offline
fallback when Redrob Console is unreachable. The agent reaches it as the
`llamacpp.router` provider: `http://127.0.0.1:8081`, model alias `router`.

Two ways to serve it, same contract:

| Image | Server | Config | Models |
|---|---|---|---|
| Redrob OS (bootc) | podman quadlet `image/agent/etc/containers/systemd/redrob-inference.container` running the upstream `ghcr.io/ggml-org/llama.cpp` server image, pinned by digest | the quadlet file itself (`Exec=` line) | `/var/lib/redrob/inference/models/router.gguf` |
| Buildroot (legacy) | `supervisor/` (`redrob-local-inference`) launching `/usr/bin/llama-server` | `deploy/config/local-inference.toml` | `/mnt/data/redrob/models/*.gguf` |

## Redrob OS: enable / disable

Off by default. The quadlet has two `ConditionPathExists=`: the enable flag and
the model file. Both must exist.

```sh
sudo install -m 0644 router.gguf /var/lib/redrob/inference/models/router.gguf
sudo touch /var/lib/redrob/inference/enabled
sudo systemctl start redrob-inference.service      # pulls the server image on first start
curl -s http://127.0.0.1:8081/v1/models            # {"data":[{"id":"router",...}]}
```

Disable: `sudo rm /var/lib/redrob/inference/enabled && sudo systemctl stop redrob-inference.service`.
The unit is `WantedBy=multi-user.target`, so after enabling it also starts on
every boot; with the flag removed it is skipped (condition failed, not an error).

Tuning (`--ctx-size`, `--threads`, memory caps) is an `/etc/containers/systemd/`
edit of the quadlet followed by `systemctl daemon-reload`; `/etc` is persistent.

## Security properties (bootc)

- Loopback only: `Network=host` plus `LLAMA_ARG_HOST=127.0.0.1`; the unit's
  cgroup has `IPAddressDeny=any` / `IPAddressAllow=localhost`, so the model can
  answer the agent but reach nothing off-box.
- Read-only rootfs, model mounted `:ro`, runs as uid 65534, all capabilities
  dropped, `NoNewPrivileges`.
- `MemoryHigh=1500M` / `MemoryMax=2G` for a ~1B Q4 model with an 8k context.

## Hailo / accelerators

Unchanged from `module.yaml`: Hailo accelerates vision (CNN) models only and is
not an LLM backend; the router LLM runs on CPU (or a remote GPU). See
`docs/open-questions.md` Q3.
