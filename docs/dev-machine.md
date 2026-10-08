# Dev machine

Measured 2026-10-08 on the current x86_64 build host (EC2, Ubuntu 26.04).

| Requirement | State | Note |
|---|---|---|
| 8+ cores, 32 GB RAM, 200 GB disk | ok | 32 cores, 123 GB, 495 GB free |
| KVM | missing | no `/dev/kvm`, no vmx/svm flags: nested virtualisation is off on this instance |
| sudo | blocked | `no new privileges`; everything below is installed under `$HOME` |
| user namespaces | blocked | `apparmor_restrict_unprivileged_userns=1`, no `newuidmap`: rootless Podman/Docker and gVisor cannot run here |
| Docker / Podman / gVisor / USBGuard / libvirt | not installable | need root or user namespaces |

Consequence: L0 (host binaries), Buildroot builds and QEMU TCG boots run here.
L2 (KVM), sandbox tests (Podman + gVisor) and binfmt arm64 containers need a
Linux machine with root and KVM.

## Toolchain (all under `$HOME`, see `scripts/dev-env.sh`)

| Tool | Location | Version |
|---|---|---|
| Rust | `~/.cargo` | 1.99.0 stable, targets x86_64 + aarch64-unknown-linux-gnu |
| aarch64 cross linker | `~/.local/toolchains/arm-gnu-toolchain-14.3.rel1-x86_64-aarch64-none-linux-gnu` | GCC 14.3.1, wired in `~/.cargo/config.toml` |
| QEMU | `~/.local/qemu` | 10.2.4, `x86_64-softmmu` + `aarch64-softmmu`, TCG only, VNC, slirp (static), internal fdt, `qemu-img` |
| llama.cpp | `~/.local/src/llama.cpp/build/bin` | CPU build, `llama-server`, `llama-cli`, `llama-bench` |
| GNU coreutils shim | `~/.local/gnu-coreutils/bin` | symlinks `gnu*` -> plain names; Buildroot rejects uutils `install` |
| cmake / ninja / meson / pigz | `~/.local/bin` | pigz unpacked from the Ubuntu .deb |

QEMU was configured with `--python=` pointing at a Python 3.12 that has pip,
because the system Python 3.14 lacks `ensurepip` and QEMU's configure needs a venv.

## Buildroot host check

`os/buildroot/support/dependencies/dependencies.sh` passes with
`scripts/dev-env.sh` sourced. Not present and not required by that check:
`makeinfo`, `help2man`, `graphviz`, `ncurses` headers (menuconfig only).

## Known blockers for `os/` builds here

The upstream `hassio` package pulls Supervisor container images at build time with
`docker` / `skopeo` (`buildroot-external/package/hassio/*.sh`). That package is
replaced by the Redrob agent in Stage 4, so it is dropped rather than ported.

## Lint gates (CI `lint.yml`, run locally before a PR)

```sh
python3 scripts/lint-modules.py      # modules/*/module.yaml against modules/README.md schema
scripts/verify-units.sh              # deploy/systemd/*.service via systemd-analyze in a stub root
scripts/l0-broker.sh test            # credential broker: cargo test (unit + integration)
shellcheck -S warning scripts/*.sh deploy/firstboot/redrob-firstboot \
  os/buildroot-external/scripts/redrob-post-build.sh \
  os/buildroot-external/package/redrob-data/create-data-partition.sh
```

Only our own files are linted; `os/buildroot`, `agent/` and upstream HAOS scripts
keep their own CI. A full image build is not in CI yet: a hosted runner has 14 GB
disk and a 6 h job limit, and the Buildroot output here is larger than that. Measure
before enabling.
