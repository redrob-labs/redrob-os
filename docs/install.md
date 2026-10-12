# Install Redrob OS

Redrob OS ships as a disk image you write to a USB stick or disk, then boot.
There is no in-place installer yet. Three steps.

Latest download: [Releases](https://github.com/redrob-labs/redrob-os/releases/latest).
x86-64 only for now.

## 1. Download

From the latest release, grab:

- `redrob_generic-x86-64-<version>.img.xz` — the image
- `redrob_generic-x86-64-<version>.img.xz.sha256` — its checksum

## 2. Flash

**Easy (recommended): [balenaEtcher](https://etcher.balena.io)**

1. Open Etcher, click **Flash from file**, pick the `.img.xz` (no need to unzip).
2. Click **Select target**, pick your USB stick or disk.
3. Click **Flash**.

Etcher verifies the write for you.

**Command line (Linux/macOS)**

Find the target device first (`lsblk` on Linux, `diskutil list` on macOS), then:

```sh
# verify the download matches the checksum
sha256sum -c redrob_generic-x86-64-<version>.img.xz.sha256

# write it — REPLACE /dev/sdX with YOUR device, double-check it
xz -dc redrob_generic-x86-64-<version>.img.xz | sudo dd of=/dev/sdX bs=4M status=progress oflag=sync
```

> **Warning.** `dd` erases the whole target device with no undo. Writing to the
> wrong device destroys that disk. Confirm `/dev/sdX` with `lsblk` before running.

## 3. Boot

Plug the drive into the target machine and power on, selecting that drive in the
boot menu if needed.

First boot sets everything up on its own — device id, signing key, hostname
`redrob-<id6>`. Nothing to type.

To connect it to your account, pair a phone with the QR code / pairing code shown
on screen. On headless hardware the same code is printed over the serial console.

## After install

- Updates are over-the-air (A/B slots with automatic rollback) — no reflashing.
- Korean: [install.ko.md](./install.ko.md).
