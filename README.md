# Redrob OS

Agentic OS for Raspberry Pi 5. A resident agent runs tasks (including coding) inside a
sandbox on behalf of the user, with channels such as Slack, Discord and Google connected.
GPU/accelerator, display, keyboard and USB are attachable modules.

한국어: [README.ko.md](./README.ko.md)

Monorepo layout:

| Path | Role | Upstream | License |
|---|---|---|---|
| `os/` | Buildroot-based read-only OS, A/B OTA, module (add-on) structure | [home-assistant/operating-system](https://github.com/home-assistant/operating-system) via `git subtree` | Apache-2.0 |
| `agent/` | Resident agent runtime, channels, cron, secrets | [zeroclaw-labs/zeroclaw](https://github.com/zeroclaw-labs/zeroclaw) via `git subtree` | MIT OR Apache-2.0 (we use Apache-2.0) |
| `modules/` | display, usb-broker, credential-broker, local-inference | new | Apache-2.0 |
| `tools/` | small on-device helpers (`redrob-pairing`: first-boot pairing banner) | new | Apache-2.0 |
| `docs/` | requirements, decisions, test plans | new | Apache-2.0 |

Redrob Code (coding tool run inside the sandbox) stays in its own repository,
[redrob-labs/redrob-code](https://github.com/redrob-labs/redrob-code).

## Upstream tracking

Each vendored tree is a `git subtree` with full history. Remotes:

```sh
git remote add upstream-os    https://github.com/home-assistant/operating-system.git
git remote add upstream-agent https://github.com/zeroclaw-labs/zeroclaw.git
```

Pull upstream changes:

```sh
git fetch upstream-os    && git subtree pull --prefix=os    upstream-os    dev
git fetch upstream-agent && git subtree pull --prefix=agent upstream-agent master
```

`os/buildroot` is a submodule (declared in the root `.gitmodules`):

```sh
git submodule update --init --depth 1 os/buildroot
```

The current upstream pins are recorded in [UPSTREAM.md](./UPSTREAM.md).

## License

Apache-2.0. See `LICENSE` and `NOTICE`. Third-party notices are listed in `NOTICE`.
