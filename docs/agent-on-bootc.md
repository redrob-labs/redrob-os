# The agent on Redrob OS (bootc image)

How the agentic half of Redrob OS is wired into the bootc image: what starts in
which order, where state lives, how tasks are sandboxed, how a phone pairs, and
how to turn it off. Everything described here is under `image/agent/`; the
Containerfile copies that tree onto `/`.

Buildroot equivalents (`deploy/`, `os/`) are mentioned only where the contract
changed.

## Boot chain

```
sysusers.d/redrob.conf      users redrob-agent, redrob-broker; group redrob-sandbox
tmpfiles.d/redrob.conf      /var/lib/redrob/{identity,agent,broker,audit,usb,inference}, /run/redrob
        │
redrob-firstboot.service    once per device (ConditionPathExists=!/var/lib/redrob/identity/device-id)
        │                   device id, ed25519 device key, agent config, hostname redrob-<id6>
        ├── redrob-broker.service       credential broker, /run/redrob/broker.sock
        ├── redrob-usb-broker.service   USB default-deny, /run/redrob/usb.sock
        ├── redrob-inference.service    (quadlet, only if enabled) router model on :8081
        └── podman.socket               task sandbox API
                │
        redrob-agent.service            /usr/bin/redrob-agent daemon  (user redrob-agent)
                │                       gateway :42617, channels (Slack/Discord/Google), relay
                ├── redrob-pairing.timer → redrob-pairing.service   console banner + pairing.json, every 30 s
                │       └── (desktop) /etc/xdg/autostart/redrob-pairing-notify.desktop → desktop notification
                └── greenboot required.d/50-redrob-agent.sh   boot is GREEN only if /health answers
```

Units are enabled by the preset `usr/lib/systemd/system-preset/80-redrob-agent.preset`
(bootc images use presets, not baked `.wants/` symlinks).

### First boot

`/usr/libexec/redrob/firstboot` (ported from `deploy/firstboot/redrob-firstboot`):

1. `/var/lib/redrob/identity/device-id` -- random UUID, kept for the life of the device.
   Written last, atomically; the unit's condition keys on it.
2. `/var/lib/redrob/identity/device.key` / `device.pub` -- ed25519 via openssl,
   key mode 0640, group `redrob-broker`.
3. `/var/lib/redrob/agent/config.toml` -- copied once from `/etc/redrob/agent.toml`.
   The agent edits its copy (`redrob-agent config set ...`); `/etc/redrob/agent.toml`
   is what *new* devices start from.
4. Hostname `redrob-<first 6 hex of the id>` -- only if the installer left
   `localhost(.localdomain)`/empty; a name the user chose in Anaconda is kept.

Unlike Buildroot it runs at normal boot time (users exist, hostnamed is up), so
there is no `redrob-state-dirs.service`: tmpfiles owns the tree.

### Health and rollback

`usr/lib/greenboot/check/required.d/50-redrob-agent.sh` waits up to 5 min for
`GET http://127.0.0.1:42617/health`. No answer = RED boot: greenboot reboots,
and after `GREENBOOT_MAX_BOOT_ATTEMPTS` bootc rolls back to the previous image.
If the agent is *disabled* (`systemctl is-enabled` not enabled) the check passes
-- turning the agent off is not a broken boot. This replaces
`redrob-mark-good.service` (rauc). The image must install `greenboot`.

## State paths

| Buildroot | bootc | Owner | What |
|---|---|---|---|
| `/mnt/data/redrob/identity` | `/var/lib/redrob/identity` | root (key: group redrob-broker) | device-id, device.key/pub |
| `/mnt/data/redrob/agent` | `/var/lib/redrob/agent` | redrob-agent 0750 | config.toml, .secret_key |
| `.../agent/data` | `/var/lib/redrob/agent/data` | redrob-agent 0700 | sqlite (sessions, cron, memory, devices), gateway-admin.token |
| `.../agent/workspace` | `/var/lib/redrob/agent/work` | redrob-agent | task workspace, bind-mounted into sandboxes as `/workspace` |
| `/mnt/data/redrob/broker` | `/var/lib/redrob/broker` | redrob-broker 0700 | encrypted credential store |
| `/mnt/data/redrob/audit` | `/var/lib/redrob/audit` | redrob-broker 0750 | `broker-YYYY-MM-DD.jsonl`, usb audit |
| `/mnt/data/redrob/usb` | `/var/lib/redrob/usb` | root | `policy.json` |
| `/mnt/data/redrob/models` | `/var/lib/redrob/inference/models` | root | `router.gguf`; `../enabled` flag |
| `/etc/redrob/{agent,broker}.toml` | same | root | templates / broker config (persistent `/etc`) |
| `/run/redrob/{broker,usb}.sock` | same | redrob-broker 0750 | sockets; agent is in group redrob-broker |

Environment the agent gets (`redrob-agent.service`): `ZEROCLAW_CONFIG_DIR=/var/lib/redrob/agent`,
`ZEROCLAW_DATA_DIR=/var/lib/redrob/agent/data`, `ZEROCLAW_WORKSPACE=/var/lib/redrob/agent/work`,
`DOCKER_HOST=unix:///run/podman/podman.sock`.

bootc keeps `/var` and `/etc` across updates and rollbacks; `/usr` is the image.
Nothing under `/var/lib/redrob` is in the image.

## Task sandbox: podman + gVisor

What the agent actually does today (`agent/crates/zeroclaw-config/src/platform/docker.rs`,
`[runtime] kind = "docker"`): it resolves the executable named **`docker`** on
`PATH` and runs

```
docker run --rm --init --interactive --network none --memory 2048m --cpus 2
           --read-only --env LANG --env TZ --env PATH
           --volume /var/lib/redrob/agent/work:/workspace:rw --workdir /workspace
           ghcr.io/redrob-labs/redrob-os:sandbox sh -c '<command>'
```

There is **no `--runtime` flag** and no podman awareness anywhere in `agent/`
(`grep -ri 'runsc\|gvisor\|podman' agent/src agent/crates/*/src` finds nothing
but a cgroup heuristic). So the OS supplies the pieces:

| Piece | File | Note |
|---|---|---|
| `docker` CLI | package `podman-docker` | `/usr/bin/docker` shim → `podman`. **Image build: add `podman-docker` (and `podman`, `catatonit` for `--init`) to the dnf install list.** |
| API socket | `podman.socket` (preset) + `usr/lib/systemd/system/podman.socket.d/50-redrob-sandbox.conf` | `/run/podman/podman.sock`, group `redrob-sandbox`, 0660. `DOCKER_HOST` in the agent unit points there. |
| runsc as default runtime | `usr/lib/systemd/system/podman.service.d/50-redrob-sandbox.conf` → `CONTAINERS_CONF_OVERRIDE=/etc/redrob/containers-sandbox.conf` | `[engine] runtime = "runsc"`, `label = false`. Scoped to `podman.service` only: quadlets and users' rootless podman/toolbox keep crun. |
| runsc binary | image build (`image/pins.env` `RUNSC_*`, installed at `/usr/bin/runsc`) | owned by the image agent; this overlay only references it. |
| sandbox image | `etc/containers/systemd/redrob-sandbox.image` | pre-pulls `ghcr.io/redrob-labs/redrob-os:sandbox` into root podman storage on first boot (not a bootc bound image: that breaks image-builder and offline installs). |
| agent config | `etc/redrob/agent.toml` `[runtime.docker]` | image, `network = "none"`, 2 GiB / 2 CPUs, read-only rootfs, workspace allow-list. |

Design notes and known limits:

- `docs/design/sandbox.md` planned **rootless** podman as `redrob-agent`. Rootless
  needs the setuid `newuidmap`/`newgidmap` and user-namespace setup, which the
  agent unit's `NoNewPrivileges=yes` and empty capability set forbid, and gVisor's
  rootless mode has its own caveats. This port therefore uses the **root** podman
  API socket: gVisor (`runsc`) is the boundary for the *tasks*; the agent process
  holds a root-equivalent socket and is trusted with it (membership of
  `redrob-sandbox` is the agent only; `/run/podman/podman.sock` is also in the
  agent's `forbidden_paths` so tools cannot touch it directly). Moving to rootless
  is a follow-up, not a config flip.
- Files a task writes under `/workspace` are created by root-podman + runsc, i.e.
  owned by uid 0 on the host; the agent (uid redrob-agent) can read them (0755
  dirs) but not modify them afterwards. Same limitation as the Buildroot design.
  Fix candidates: `userns = "auto"` in `containers-sandbox.conf`, or the sandbox
  image running as the agent's uid.
- `label = false`: runsc does not implement SELinux labelling of its sandbox and
  the bind-mounted workspace (`var_lib_t`) would otherwise be unwritable.
- Sandboxes are accounted to `podman.service`, not to the agent's 1 GiB cap.

## Credential broker, USB broker

Unchanged binaries and sockets; only paths moved (table above).

- `redrob-broker.service` -- `/usr/bin/redrob-broker --config /etc/redrob/broker.toml`,
  user `redrob-broker`, egress proxy on `127.0.0.1:3128` that the agent is pointed
  at (`[proxy]` in agent.toml), scoped `POST /v1/call` over `/run/redrob/broker.sock`.
- `redrob-usb-broker.service` -- root, `authorized_default=0` on all host
  controllers, approvals over `/run/redrob/usb.sock`, read-only mounts under
  `/run/media/redrob`. New: `usr/lib/udev/rules.d/70-redrob-usb-broker.rules` arms
  hot-added host controllers at udev time and sets `UDISKS_AUTO=0` on removable
  USB block devices so the desktop does not auto-mount read-write over the broker's
  read-only mount (the device stays visible; manual mount still works).

## Local inference

`etc/containers/systemd/redrob-inference.container` -- the upstream llama.cpp
server image `ghcr.io/ggml-org/llama.cpp:server-b11718`, pinned by its multi-arch
index digest `sha256:4b7be0376002b4450a357a109bab804093ff9251193cc75ed4ce0cb39953ff5c`
(amd64 + arm64). Loopback `:8081`, alias `router`, 8k context, egress denied on
the unit's cgroup, read-only, uid 65534. Starts only when **both**
`/var/lib/redrob/inference/enabled` and `/var/lib/redrob/inference/models/router.gguf`
exist. See `modules/local-inference/README.md` for enable/disable. The Buildroot
supervisor (`modules/local-inference/supervisor`) is not installed on bootc.

## Pairing from a phone

Same flow as before (`docs/remote-access.md`, LAN-direct). The device gateway
listens on all interfaces at `:42617`; every route is gated by the one-time
pairing token, so an unpaired host on the LAN gets nothing.

1. Put the phone on the **same Wi-Fi** as the device.
2. Get the code from the device:
   - **Console / serial / SSH login prompt:** the banner with the QR
     (`/run/issue.d/50-redrob-pairing.issue`, refreshed every 30 s by
     `redrob-pairing.timer`; also `journalctl -u redrob-pairing`).
   - **Desktop (Redrob session):** a notification "Redrob OS -- pair this device" with the
     code, the address and the `redrob://pair?...` link appears after login and
     stays current while the device is unpaired
     (`/etc/xdg/autostart/redrob-pairing-notify.desktop` → `redrob-pairing --notify`,
     which reads `/run/redrob-pairing/pairing.json`). It disappears on its own
     once paired. Needs `libnotify` (`notify-send`) in the image.
3. Scan the QR or enter the code in the Redrob app. It encodes
   `redrob://pair?device=<id>&host=<device-ip>:42617&code=<code>&fp=<sha256>`.
4. The phone pairs and opens the dashboard at `https://<device-ip>:42617`,
   pinning the certificate fingerprint `fp`.

This listener serves HTTPS with a self-provisioned certificate (see
`docs/remote-access.md`, "LAN-direct"); loopback callers such as the greenboot
health check may still use plain HTTP. From anywhere else use the relay
(`docs/remote-access.md`, "Relay"); the relay path is unchanged by the bootc move.

## Turning the agent off (and back on)

```sh
sudo systemctl disable --now redrob-agent.service redrob-pairing.timer
```

That is persistent (`/etc/systemd/system`), survives image updates, and the
greenboot check skips a disabled agent so boots stay GREEN. The brokers may stay
up (they do nothing without the agent) or go too:

```sh
sudo systemctl disable --now redrob-broker.service redrob-usb-broker.service
```

Back on: `sudo systemctl enable --now redrob-agent.service redrob-pairing.timer`.
To wipe the agent's identity and pair again from scratch:
`sudo systemctl stop redrob-agent && sudo rm -rf /var/lib/redrob/agent /var/lib/redrob/identity`
then reboot -- `redrob-firstboot` runs again.

## What the image build must provide (for `image/Containerfile`)

- Packages: `podman podman-docker catatonit greenboot libnotify openssl curl`
  (`curl` for the greenboot check; `openssl` for the device key).
- Binaries compiled in CI: `/usr/bin/redrob-agent`, `/usr/bin/redrob-broker`,
  `/usr/bin/redrob-usb-broker`, `/usr/bin/redrob-pairing` (`tools/redrob-pairing`,
  now with `--notify`).
- `/usr/bin/runsc` from the pinned gVisor release (`image/pins.env`).
- `COPY image/agent/ /` then `systemctl preset-all` (or rely on first-boot preset
  application) so `80-redrob-agent.preset` takes effect.
- An icon named `redrob-os` in the hicolor theme (branding overlay) for the
  pairing notification; falls back to no icon otherwise.

## Not verified

This overlay was written against the Buildroot units, the agent source and the
greenboot/quadlet documentation; **no image was built and nothing was booted**.
Specifically unverified: quadlet key acceptance by the Fedora 44 podman
generator, `notify-send --print-id`/`--replace-id` on the shipped libnotify, the
`UDISKS_AUTO` rule's effect on the desktop, SELinux interactions beyond
`label = false`, and that `CONTAINERS_CONF_OVERRIDE` is honoured by the podman
version in the base image. The one build is the PR's CI run.
