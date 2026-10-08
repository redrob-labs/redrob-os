#!/usr/bin/env bash
# Source this before building: `. scripts/dev-env.sh`
# Everything lives under $HOME; no system packages are required.

# GNU coreutils first: Ubuntu 26.04 ships uutils by default and Buildroot
# refuses its `install` (uutils/coreutils#12166).
[ -d "$HOME/.local/gnu-coreutils/bin" ] && PATH="$HOME/.local/gnu-coreutils/bin:$PATH"

# Rust (rustup, target aarch64-unknown-linux-gnu, linker in ~/.cargo/config.toml)
[ -d "$HOME/.cargo/bin" ] && PATH="$HOME/.cargo/bin:$PATH"

# Arm GNU cross toolchain (aarch64-none-linux-gnu-*)
ARM_GNU="$(ls -d "$HOME"/.local/toolchains/arm-gnu-toolchain-*-x86_64-aarch64-none-linux-gnu 2>/dev/null | tail -1)"
[ -n "$ARM_GNU" ] && PATH="$ARM_GNU/bin:$PATH"

# QEMU (TCG only; this host has no /dev/kvm)
[ -d "$HOME/.local/qemu/bin" ] && PATH="$HOME/.local/qemu/bin:$PATH"

# llama.cpp
[ -d "$HOME/.local/src/llama.cpp/build/bin" ] && PATH="$HOME/.local/src/llama.cpp/build/bin:$PATH"

# cmake / ninja / meson / pigz installed under ~/.local/bin
PATH="$HOME/.local/bin:$PATH"

export PATH
export CROSS_COMPILE=aarch64-none-linux-gnu-

# Buildroot (os/): the upstream defconfigs point at /cache from the build
# container; keep downloads and ccache under $HOME instead.
export BR2_DL_DIR="$HOME/.local/cache/redrob-os/dl"
export BR2_CCACHE_DIR="$HOME/.local/cache/redrob-os/cc"
export FORCE_UNSAFE_CONFIGURE=1
