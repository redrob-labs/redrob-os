# Install Redrob OS

Redrob OS ships as an installer ISO for PCs and laptops (x86-64) and as a raw disk
image for the Raspberry Pi 5 (aarch64). Both come from the latest
[release](https://github.com/redrob-labs/redrob-os/releases/latest), with a `.sha256`
checksum next to each file.

## PC or laptop

### 1. Download

- `redrob-os-<version>-x86_64.iso` — the installer
- `redrob-os-<version>-x86_64.iso.sha256` — its checksum

### 2. Write it to a USB stick (4 GB or more)

**Easy (recommended): [balenaEtcher](https://etcher.balena.io)** — Flash from file, pick
the `.iso`, pick the stick, Flash. Etcher verifies the write.

**Command line (Linux/macOS)** — find the stick first (`lsblk` on Linux, `diskutil list`
on macOS), then:

```sh
sha256sum -c redrob-os-<version>-x86_64.iso.sha256
# REPLACE /dev/sdX with YOUR stick, double-check it
sudo dd if=redrob-os-<version>-x86_64.iso of=/dev/sdX bs=4M status=progress oflag=sync
```

> **Warning.** `dd` erases the whole target device with no undo. Confirm `/dev/sdX`
> with `lsblk` before running.

### 3. Boot the stick and install

Boot from the USB stick (pick it in the firmware boot menu, usually F12, F2, Esc or
Del at power-on). The guided installer asks for:

- **Language and keyboard** (Korean is available; the system then boots in Korean).
- **Where to install.** Pick a disk. To keep Windows, choose *Install alongside* and the
  installer shrinks the Windows partition and sets up a boot menu; to replace what is
  on the disk, choose *Use the whole disk*. Secure Boot can stay on.
- **Disk encryption** (optional, recommended on laptops). You type the passphrase at
  every boot.
- **Your user account** — name, password; this account is the administrator.

Click Install, wait, remove the stick, reboot. Nothing else to configure.

## Raspberry Pi 5

Download `redrob-os-<version>-aarch64.raw.xz` and its `.sha256`, write it to a microSD
card or USB/NVMe drive with balenaEtcher (it unpacks `.xz` for you), and power on.
A keyboard, a mouse and an HDMI display are needed for the first boot (user account
and Wi‑Fi). The image grows its last partition to fill the card on first boot.

## First boot

The desktop comes up with a short first-run setup (Wi‑Fi if needed, then the Redrob
Launcher, which installs the Redrob apps into your home). The agent is already running;
to pair your phone, open the pairing QR from the Launcher or the system tray and scan
it. On a headless device the same code is printed over the serial console.

## After install

- Updates are automatic: downloaded once a day, applied at the next reboot, rolled
  back by the device itself if the new version does not boot. See
  [maintenance.md](./maintenance.md).
- Reaching the device from your phone: [remote-access.md](./remote-access.md).
- Korean: [install.ko.md](./install.ko.md).
