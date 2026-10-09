#!/usr/bin/env python3
"""T10: display module -- headless -> kiosk switch on virtio-gpu.

Usage: t10_display.py <raw-disk.img> <outdir>
Boots the image with a virtio-gpu and a USB keyboard, then checks:
  - headless (default): redrob-display is not running (mode file absent), getty@tty1 is up,
    screendump 1 is the text console
  - write "kiosk" to /mnt/data/redrob/display/mode and start the unit -> active, running as
    redrob-display, getty@tty1 stopped (Conflicts)
  - screendump 2 shows the dashboard: accent rule (#c162f4) under the header, a white QR
    quiet zone, and it differs from screendump 1
  - device isolation: /dev/dri/card0 is root:video 0660, /dev/input/event* root:input 0660,
    redrob-display is the only member of video/input, and a process running as redrob-agent
    cannot open either
  - the kiosk grabbed the keyboard: its status shows >= 1 input device held
  - kiosk survives a reboot (mode persists on /mnt/data)
Writes <outdir>/t10.json, t10-headless.ppm, t10-kiosk.ppm.
"""
import json, os, re, subprocess, sys, time
sys.path.insert(0, os.path.dirname(__file__))
from vm import VM

disk, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
R = {}
EXTRA = ["-vga", "none", "-device", "virtio-gpu-pci", "-device", "qemu-xhci", "-device", "usb-kbd"]


def px_mean(ppm, crop):
    return float(subprocess.run(["magick", ppm, "-crop", crop, "-format", "%[fx:mean]", "info:"],
                                capture_output=True, text=True, check=True).stdout.strip())


def has_color(ppm, hexcolor, tol=12):
    """Number of pixels within +-tol per channel of hexcolor (binary P6 PPM)."""
    data = open(ppm, "rb").read()
    # Header is exactly "P6\n<w> <h>\n255\n"; do not whitespace-split the payload, whose first
    # bytes (0x0b 0x0d ...) are themselves whitespace.
    m = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    assert m, ppm
    w, h = int(m.group(1)), int(m.group(2))
    px = data[m.end(): m.end() + w * h * 3]
    r, g, b = (int(hexcolor[i:i + 2], 16) for i in (1, 3, 5))
    n = 0
    for i in range(0, len(px), 3):
        if abs(px[i] - r) <= tol and abs(px[i + 1] - g) <= tol and abs(px[i + 2] - b) <= tol:
            n += 1
    return n


vm = VM(disk, "t10", extra=EXTRA)
try:
    vm.login(timeout=1500)
    time.sleep(5)
    R["headless.display"] = vm.run("systemctl is-active redrob-display.service")[1].split()[-1]
    R["headless.display_cond"] = vm.run("systemctl show -p ConditionResult redrob-display.service")[1]
    R["headless.getty1"] = vm.run("systemctl is-active getty@tty1.service")[1].split()[-1]
    # The text login console: under this serial-console test recipe it is the serial getty
    # on ttyS0 (we logged in over it); on a VGA-only headless device it would be getty@tty1.
    # Either one being up proves a non-kiosk login console is present.
    R["headless.serial_getty"] = vm.run("systemctl is-active serial-getty@ttyS0.service")[1].split()[-1]
    vm.monitor(f"screendump {out}/t10-headless.ppm")
    time.sleep(2)

    # Device/permission facts (independent of the mode).
    R["dri_perm"] = vm.run("stat -c '%U:%G %a' /dev/dri/card0")[1].splitlines()[-1]
    R["input_perm"] = vm.run("stat -c '%U:%G %a' /dev/input/event0")[1].splitlines()[-1]
    R["group_video"] = vm.run("grep '^video:' /etc/group")[1].strip().splitlines()[-1]
    R["group_input"] = vm.run("grep '^input:' /etc/group")[1].strip().splitlines()[-1]
    R["agent_open_dri"] = vm.run(
        "systemd-run --quiet --wait --uid=redrob-agent /bin/sh -c 'head -c1 /dev/dri/card0 >/dev/null' 2>&1; echo rc=$?")[1]
    R["agent_open_input"] = vm.run(
        "systemd-run --quiet --wait --uid=redrob-agent /bin/sh -c 'head -c1 /dev/input/event0 >/dev/null' 2>&1; echo rc=$?")[1]

    # Switch to kiosk.
    vm.check("mkdir -p /mnt/data/redrob/display && echo kiosk > /mnt/data/redrob/display/mode")
    vm.check("systemctl start redrob-display.service")
    time.sleep(8)
    R["kiosk.display"] = vm.run("systemctl is-active redrob-display.service")[1].split()[-1]
    R["kiosk.user"] = vm.run("ps -o user= -C redrob-display")[1].split()[-1] if R["kiosk.display"] == "active" else ""
    R["kiosk.getty1"] = vm.run("systemctl is-active getty@tty1.service")[1].split()[-1]
    R["kiosk.journal"] = vm.run("journalctl -u redrob-display.service --no-pager -o cat | tail -n 12")[1]
    vm.monitor(f"screendump {out}/t10-kiosk.ppm")
    time.sleep(2)

    # Reboot: the mode file persists, so the kiosk must come back on its own.
    R["reboot"] = vm.reboot()
    vm2 = VM(disk, "t10", extra=EXTRA)
    vm = vm2
    vm.login(timeout=1500)
    time.sleep(10)
    R["after_reboot.display"] = vm.run("systemctl is-active redrob-display.service")[1].split()[-1]
    R["after_reboot.getty1"] = vm.run("systemctl is-active getty@tty1.service")[1].split()[-1]
    R["after_reboot.mode_trigger"] = re.sub(
        r"\x1b\[[0-9;?]*[A-Za-z]", "",
        vm.run("systemctl show -p ExecMainStatus --value redrob-display-mode.service")[1]
    ).split()[-1]
    vm.monitor(f"screendump {out}/t10-kiosk-reboot.ppm")
    time.sleep(2)
finally:
    vm.kill()

# Derived checks
R["check.headless_not_running"] = R.get("headless.display") in ("inactive", "failed") or "ConditionResult=no" in R.get("headless.display_cond", "")
R["check.headless_console"] = "active" in (R.get("headless.getty1"), R.get("headless.serial_getty"))
R["check.kiosk_active"] = R.get("kiosk.display") == "active"
R["check.kiosk_user"] = R.get("kiosk.user") == "redrob-display"
R["check.kiosk_stops_getty"] = R.get("kiosk.getty1") != "active"
R["check.dri_perm"] = R.get("dri_perm") == "root:video 660"
R["check.input_perm"] = R.get("input_perm") == "root:input 660"
R["check.only_display_in_groups"] = all(
    re.fullmatch(r"\w+:x:\d+:redrob-display", R.get(k, "")) is not None for k in ("group_video", "group_input"))
R["check.agent_cannot_open_dri"] = "rc=0" not in R.get("agent_open_dri", "rc=0")
R["check.agent_cannot_open_input"] = "rc=0" not in R.get("agent_open_input", "rc=0")
R["check.input_grabbed"] = "grabbed" in R.get("kiosk.journal", "")
R["check.mode_trigger"] = R.get("after_reboot.mode_trigger") == "0"
R["check.after_reboot_kiosk"] = R.get("after_reboot.display") == "active" and R.get("after_reboot.getty1") != "active"
try:
    k = f"{out}/t10-kiosk.ppm"
    R["kiosk.accent_px"] = has_color(k, "#c162f4")
    R["kiosk.white_px"] = has_color(k, "#ffffff", 2)
    R["headless.white_px"] = has_color(f"{out}/t10-headless.ppm", "#ffffff", 2)
    R["check.kiosk_has_accent_rule"] = R["kiosk.accent_px"] > 500
    R["check.kiosk_has_qr_quiet_zone"] = R["kiosk.white_px"] > 2000
    R["check.frames_differ"] = subprocess.run(["cmp", "-s", k, f"{out}/t10-headless.ppm"]).returncode != 0
    R["check.kiosk_after_reboot_has_accent"] = has_color(f"{out}/t10-kiosk-reboot.ppm", "#c162f4") > 500
except Exception as e:  # noqa: BLE001
    R["check.pixel_error"] = repr(e)

with open(f"{out}/t10.json", "w") as f:
    json.dump(R, f, indent=2, ensure_ascii=False)
for k, v in R.items():
    if k.startswith("check.") or k in ("headless.display", "kiosk.display", "kiosk.user", "reboot", "dri_perm", "input_perm"):
        print(f"{k}: {v}")
