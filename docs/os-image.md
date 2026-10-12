# OS image (os/)

Build, without Docker, on the dev host:

```sh
. scripts/dev-env.sh                 # GNU coreutils shim, BR2_DL_DIR/BR2_CCACHE_DIR under $HOME
(cd agent && cargo build --release --bin zeroclaw --features channel-slack)
make -C os redrob_x86_64             # defconfig + full build -> os/output/images/
```

Output: `os/output/images/redrob_generic-x86-64-<version>.img.xz` (raw GPT disk),
`.raucb` (OTA bundle), `.qcow2`/`.vmdk`/`.vdi` (virtual), `data.ext4`.

### aarch64 / Raspberry Pi 5

The device binaries are prebuilt per architecture, so the aarch64 image needs the
six Redrob crates cross-built first (the defconfig points each
`BR2_PACKAGE_REDROB_*_BINARY` at `target/aarch64-unknown-linux-gnu/release/`):

```sh
. scripts/dev-env.sh
export CC_aarch64_unknown_linux_gnu=aarch64-none-linux-gnu-gcc
export AR_aarch64_unknown_linux_gnu=aarch64-none-linux-gnu-ar
for d in modules/credential-broker/broker modules/usb-broker/broker \
         modules/local-inference/supervisor modules/display/kiosk tools/redrob-pairing; do
  (cd "$d" && cargo build --release --target aarch64-unknown-linux-gnu)
done
(cd agent && cargo build --release --target aarch64-unknown-linux-gnu -p zeroclaw --bin zeroclaw)
make -C os O=output-rpi5 redrob_rpi5_64   # fresh Buildroot aarch64 toolchain + full image
```

Output: `os/output-rpi5/images/redrob_rpi5-64-<version>.img.xz` (+ `.raucb`,
`rootfs.erofs`, rpi5 DTBs, `boot.vfat`, `kernel.img`). A separate `O=output-rpi5`
keeps the x86 `output/` toolchain intact.

Linker prereq: `agent/.cargo/config.toml` names the Debian linker
`aarch64-linux-gnu-gcc`, which this host does not ship. `scripts/dev-env.sh`'s
global `~/.cargo/config.toml` uses the Arm GNU `-none-linux-gnu-` name, so the
agent build needs `~/.local/bin/aarch64-linux-gnu-*` symlinks pointing at the Arm
GNU toolchain's `aarch64-none-linux-gnu-*`. The other five crates link fine from
the global config.

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

## Resolved in dev5: aarch64 / Raspberry Pi 5 image

`redrob_rpi5_64_defconfig` = upstream `rpi5_64` (RPi kernel/DTB/boot chain) with
the same Redrob delta as x86 (hostname `redrob`, issue, `redrob-post-build.sh`,
Supervisor `BR2_PACKAGE_HASSIO*` dropped, the seven `redrob-*` packages +
`llama-cpp`). The six device crates are cross-built to
`aarch64-unknown-linux-gnu` and each `BR2_PACKAGE_REDROB_*_BINARY` points at that
output; `llama-cpp` cross-builds from source (`GGML_NATIVE=OFF`). Built as
`O=output-rpi5`. Verified: all six binaries + `llama-server` land in the rootfs as
aarch64 ELF and every `redrob-*` service symlink is enabled. NOT verified: a real
boot — this host has no Raspberry Pi 5 QEMU machine model, so first-run/pairing on
hardware is untested.

## Resolved in dev5: broker state-dir provisioning

Both broker crash-loops seen on the dev4 full-image boot are fixed in dev5 and
re-verified on a clean boot (`redrob-broker` and `redrob-usb-broker` both
`active`). Root cause: firstboot ran `DefaultDependencies=no` (too early for a
reliable `chown`-by-name) and the pre-built data partition baked the dirs with
the build host's orphan uid, so `mkdir -p` no-op'd over them and ownership was
never fixed; `/mnt/data/redrob/usb` was never baked at all, so the usb broker's
mount namespace failed `226/NAMESPACE`. Fix: a dedicated oneshot
`redrob-state-dirs.service` runs at multi-user time (users resolvable, data
mounted) and owns `broker`/`audit` + creates `usb` every boot; both brokers
`Requires=`/`After=` it. firstboot no longer touches those dirs.
