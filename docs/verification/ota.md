# OTA and power-cut verification (x86-64, QEMU TCG)

Date: 2026-10-08. Host: dev machine from `dev-machine.md` (no KVM, TCG only).
Images: slot A = `0.1.dev0` (`redrob_generic-x86-64-0.1.dev0.img.xz`), update bundle =
`0.1.dev1.raucb`. The only functional difference between the two builds is
`redrob-mark-good.service` (dev1), which is what T1 exercises.

Harness: a 4 GiB raw copy of the image, `console=ttyS0,115200` appended to the ESP
`cmdline.txt` so the serial getty comes up, QEMU q35/4 vCPU/2 GiB, driven over the serial
socket with pexpect. The bundle is served from the host on loopback and reaches the guest
as `10.0.2.2` (slirp). A "power cut" is `SIGKILL` of the QEMU process. Harness: `scripts/ota-harness/` (`vm.py` plus one script per scenario; needs `pexpect`,
`~/.local/qemu`, a 4 GiB raw copy of the image as `a.img` in the same directory and
`python3 -m http.server 18090 --bind 127.0.0.1` serving the bundles). Run with `-cpu max` (finding 6).

## Results

| # | Scenario | Result |
|---|---|---|
| T1 | install dev1 bundle -> reboot -> slot B boots -> `mark-good` resets TRY | pass (see below) |
| T2 | power cut at 98 % of `Copying image to rootfs.1` -> reboot | pass: boots A (dev0), `B_OK=0`, no failed units; re-install afterwards succeeds and flips `ORDER=B A B_OK=1` |
| T3a | bundle with 64 flipped bytes mid-payload | refused: dm-verity `data block 35128 is corrupted` -> rauc `Failed updating slot rootfs.1: ... Input/output error`; grubenv unchanged (`B_OK=0`) |
| T3b | bundle truncated by 1 MiB | refused before any write: `Invalid bundle format: Signature size ... exceeds bundle size`; grubenv unchanged |
| T4 | 20 x 1 MiB files written + `sync`, then a 4 MiB/file write loop, power cut after ~8 s (37 files present) | after reboot: `e2fsck 1.47.2 ... recovering journal`, `/mnt/data` mounted `rw`, 20/20 synced files `sha256sum -c` OK, 6 of the unsynced files survive (rest lost, as expected), `identity/{device-id,device.key}` intact, no failed units |

### T1 detail

Filled from the run log (`t1.out`):

| Step | Observed |
|---|---|
| boot A (dev0) | `rauc status`: `Booted from: kernel.0 (A)`, both slots `boot status: bad`, grubenv `ORDER=A B A_OK=1 A_TRY=1 B_OK=0` (finding 1) |
| `curl` bundle to `/mnt/data/redrob/ota/`, `rauc install <path>` | `100% Installing done.` / `Installing ... succeeded`; grubenv `ORDER=B A A_OK=1 A_TRY=1 B_TRY=0 B_OK=1`; rootfs.1 `boot status: good` |
| reboot -> boot B (dev1, `-cpu qemu64`) | `rauc.slot=B`, `VERSION="0.1.dev1"`; **agent `SIGILL` every 1.3 s, 10 restarts** (finding 6); `redrob-mark-good` fails as designed (`agent not healthy, slot stays unmarked`); grubenv `B_TRY=1` |
| 2 more boots on B with the agent still crashing | `B_TRY=2`, then `B_TRY=3` |
| next boot | **GRUB falls back to slot A** (`rauc.slot=A`, `VERSION="0.1.dev0"`, grubenv `B_TRY=3 B_OK=1`) -- rollback works without any help from userspace |
| boot B on a second disk with `-cpu max` (agent runs) | `redrob-mark-good` `active` at 18 s: `rauc status: marked slot kernel.1 as good`; grubenv `B_TRY=0 B_OK=1`; rootfs.1 `good`; agent `NRestarts=0`, `/health` `status: ok`; no failed units |

So the two halves of the A/B contract both hold: a slot whose agent never becomes healthy is abandoned
after three tries, and a slot whose agent is healthy is kept.

## Findings that change the product

1. **dev0 never marks a slot good.** GRUB raises `<slot>_TRY` on every boot and gives up at 3.
   Upstream resets it from the Supervisor; without one, `rauc status` already reports
   `No bootable slot found in ORDER 'A B'` / `boot status: bad` on the first boot, and a dev0
   device would stop booting after its third power cycle. `deploy/systemd/redrob-mark-good.service`
   (in dev1) runs `rauc status mark-good` only after the agent answers `/health`, so a boot where
   the agent never comes up is still counted as a failed try.
2. **`rauc install <http-url>` fails with `Maximum file size exceeded`.** RAUC caps URL installs
   at 8 MiB by default; upstream never saw it because the Supervisor downloads to disk first.
   `redrob-post-build.sh` now sets `max-bundle-download-size=1 GiB`. The agent should still
   download to `/mnt/data/redrob/ota/` and install the local path (what the harness does): the
   download then survives a reboot and does not land in the 2 GiB tmpfs.
3. **Clean `systemctl reboot` takes 10-20 minutes.** `journalctl -b -1` of a rebooted guest shows
   PID 1 advancing exactly one stop step every ~30.2 s (`Stopping Home Assistant CLI` at 294 s,
   `haos-persists` 324 s, `redrob-mark-good` 354 s, `RAUC` 385 s, `Redrob Agent` 415 s, `RPC Bind`
   445 s, `Hostname` 475 s, `logind` 506 s, `random-seed` 536 s ...). Same with and without a serial
   login session (T6: `systemctl reboot --no-block; exit` -> 582 s to `reboot: Restarting system`).
   Something PID 1 does per unit during shutdown blocks for a 30 s timeout. Not root-caused; open
   item, and it may be TCG-only. The harness hard-resets after a timeout and records that it did.
4. Login MOTD still said "Home Assistant OS" -- replaced at post-build.
5. `redrob-mark-good` gave the agent 60 s. On the first boot the agent renders config and keys, and
   small CPUs are slow, so a healthy device could be counted as a failed try. Raised to 5 min
   (`TimeoutStartSec=6min`). Unit-verified only; it ships with the next image build.
6. **The agent dies with `SIGILL` on QEMU's default `qemu64` CPU** (no SSSE3/SSE4/POPCNT/AVX); with
   `-cpu max` it is healthy in 5 s with 0 restarts. The `systemd-coredump` seen on every earlier
   boot was this. `cargo build --release` has no `target-cpu` override, so some dependency assumes
   > x86-64 baseline. Any real x86 target is x86-64-v3 or better, so this is a harness setting
   (`-cpu max` from now on), but the aarch64 build should be checked the same way.
7. `redrob-agent.service` points at `/mnt/data/redrob/...` while `docs/design/*.md` and
   `modules/*/module.yaml` still say `/data/...`. One of them has to move; the OS side is the one
   that is tested.

## Not covered here

- A real power cut on hardware (QEMU `SIGKILL` loses the page cache the same way, but not disk
  firmware write caches).
- KVM timing; everything above ran under TCG, so boot-to-agent is minutes, not seconds.
- aarch64 (`rpi5_64`) and the tryboot bootloader path.
- Upgrade across a kernel ABI change, bundle signed with the production key (dev self-signed cert
  from `.dev-signing/` throughout).
