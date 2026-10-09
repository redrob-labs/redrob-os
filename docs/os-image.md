# OS image (os/)

Build, without Docker, on the dev host:

```sh
. scripts/dev-env.sh                 # GNU coreutils shim, BR2_DL_DIR/BR2_CCACHE_DIR under $HOME
(cd agent && cargo build --release --bin zeroclaw --features channel-slack)
make -C os redrob_x86_64             # defconfig + full build -> os/output/images/
```

Output: `os/output/images/redrob_generic-x86-64-<version>.img.xz` (raw GPT disk),
`.raucb` (OTA bundle), `.qcow2`/`.vmdk`/`.vdi` (virtual), `data.ext4`.

## What differs from upstream HAOS

| Area | Change | Where |
|---|---|---|
| Target | `redrob_x86_64_defconfig` = generic_x86_64 minus the Supervisor (`BR2_PACKAGE_HASSIO*`), plus `redrob-agent`, `redrob-data`; hostname `redrob`, issue "Welcome to Redrob OS" | `buildroot-external/configs/` |
| Agent | prebuilt `agent/target/release/zeroclaw` installed as `/usr/bin/redrob-agent`, unit, config template, first-boot script; users `redrob-agent`, `redrob-broker` | `buildroot-external/package/redrob-agent/`, `deploy/` |
| Data partition | built from a directory with `mke2fs -d` (no root, no Docker); tree `redrob/{agent,broker,identity,audit,models,modules}` plus `identity/.firstboot` | `buildroot-external/package/redrob-data/` |
| First boot | `redrob-firstboot.service` (before the agent, after `/mnt/data`): device UUID, ed25519 key, config render, hostname `redrob-<id6>`; nothing identity-related is in the image, post-build fails if it is | `deploy/firstboot/redrob-firstboot` |
| Identity | `meta`: `HAOS_NAME="Redrob OS"`, `HAOS_ID="redrob"`, version `0.1.dev0`; os-release vendor fields; RAUC compatible `redrob-generic-x86-64` | `buildroot-external/meta`, `scripts/redrob-post-build.sh` |
| Partition labels | `hassos-*` -> `redrob-*` everywhere (boot, kernel0/1, system0/1, bootstate, overlay, data, config, zramswap). Done now because labels, RAUC compatible string and image id are what an installed base cannot change later | 27 files under `buildroot-external/` |
| Supervisor units | `haos-supervisor`, `haos-apparmor`, `haos-bt-cache.timer` masked at post-build; overlay files untouched so the subtree still merges | `scripts/redrob-post-build.sh` |
| Build env | downloads and ccache under `$HOME/.local/cache/redrob-os/` instead of the container's `/cache` | `scripts/dev-env.sh` |

Partition layout (GPT, from `genimage/partitions-os-gpt.cfg`): `redrob-boot` (ESP, 32M),
`redrob-kernel0/1` (24M each), `redrob-system0/1` (erofs, 256M each),
`redrob-bootstate` (8M), `redrob-overlay` (ext4, 96M), `redrob-data` (ext4, grows to disk).
A/B slots and rollback come from RAUC + GRUB as upstream.

## Deferred

- Boot splash: done in dev2 as the kernel logo (`BR2_LINUX_KERNEL_CUSTOM_LOGO_PATH` ->
  `branding/boot-logo.png`, Buildroot turns on `CONFIG_LOGO`/`CLUT224`); fbcon paints it
  top-left. A full-screen splash stays with the display module (kiosk).
- Docker engine is still in the image (the agent's `[runtime] kind = "docker"` talks to a
  Docker-compatible socket). Swap to Podman + gVisor is the sandbox work item.
- `BR2_PACKAGE_OS_AGENT` (upstream D-Bus agent) stays; audit whether the Redrob agent needs it.
- aarch64 (`rpi5_64`) defconfig: same package set, agent binary from the cross build.

## Known issues (dev4, from `docs/verification/measure.md`)

- `redrob-broker` crash-loops at first boot: `Permission denied` creating
  `/mnt/data/redrob/broker/.secret_key`. firstboot's chown of that dir is not
  taking effect against the pre-built data partition. Fix in the broker/firstboot
  deploy, not local-inference.
- `redrob-usb-broker` fails `226/NAMESPACE`: it binds `/mnt/data/redrob/usb`
  before firstboot creates it. Needs `After=redrob-firstboot.service` or an
  `ExecStartPre`/`RuntimeDirectory`.
- Both were previously verified only on the L0 host; dev4 is the first full-image
  boot to exercise them.
