#!/usr/bin/bash
# 10-pins.sh -- install gVisor (runsc) as a podman OCI runtime for the agent sandbox.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
#
# Pinned in image/pins.env (GVISOR_*). Upstream ships one tar.zstd per arch:
#   runsc, containerd-shim-runsc-v1, gvisor-bin/{gvisor_sentry,checkpointgofer,...}
# runsc insists on finding its sidecars in a `gvisor-bin/` directory next to its
# own (symlink-resolved) executable, so the whole tree goes to /usr/libexec/gvisor
# and /usr/bin gets symlinks. podman is told the real path in containers.conf.d.

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"
load_pins

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

fetch_pinned GVISOR "${work}/gvisor.tar.zstd"

install -d -m 0755 /usr/libexec/gvisor
tar --zstd --extract --file="${work}/gvisor.tar.zstd" --directory=/usr/libexec/gvisor \
    --no-same-owner --no-same-permissions
chmod 0755 /usr/libexec/gvisor/runsc /usr/libexec/gvisor/containerd-shim-runsc-v1
chmod -R a+rX /usr/libexec/gvisor/gvisor-bin
ln -sfn ../libexec/gvisor/runsc /usr/bin/runsc
ln -sfn ../libexec/gvisor/containerd-shim-runsc-v1 /usr/bin/containerd-shim-runsc-v1

# Register the runtime with podman. crun stays the default; the agent asks for
# `--runtime runsc` per task. Keep in sync with image/agent/ sandbox config.
install -d -m 0755 /etc/containers/containers.conf.d
cat > /etc/containers/containers.conf.d/50-redrob-runsc.conf <<EOF
# Redrob OS: gVisor ${GVISOR_VERSION} as an additional OCI runtime (agent task sandbox).
# Installed by image/scripts/10-pins.sh; the default runtime is unchanged.
[engine.runtimes]
runsc = ["/usr/libexec/gvisor/runsc"]
EOF

# Record what was installed (read by greenboot check + `redrob-os --version` style tooling).
install -d -m 0755 /usr/share/redrob
printf 'GVISOR_VERSION=%s\n' "${GVISOR_VERSION}" >> /usr/share/redrob/pins-installed.env

echo "gVisor ${GVISOR_VERSION} (${ARCH}) installed to /usr/libexec/gvisor"
