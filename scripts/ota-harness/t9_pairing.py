#!/usr/bin/env python3
"""T9: first-boot pairing banner and boot branding on a fresh dev2 image.

Usage: t9_pairing.py <raw-disk.img> <outdir>
Checks, on the serial console after first boot:
  - redrob-pairing.timer active, redrob-pairing.service ran (oneshot, result success)
  - /run/issue.d/50-redrob-pairing.issue exists: device id, host:port, the agent's code, QR block
  - /run/redrob-pairing/pairing.json parses and matches the banner
  - /run/redrob stays owned by the broker user (the pairing unit must not re-own it)
  - /etc/issue shows the product line; the banner is shown on the serial login prompt
  - tty1 is a plain getty (ha-cli@tty1 masked, getty@tty1 enabled)
  - kernel has CONFIG_LOGO (/proc/config.gz) and the VGA screendump has a non-black
    top-left tile where fbcon paints the logo
Writes <outdir>/t9.json and <outdir>/t9-vga.ppm.
"""
import json, os, re, subprocess, sys, time
sys.path.insert(0, os.path.dirname(__file__))
from vm import VM

disk, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
R = {}
vm = VM(disk, "t9", extra=["-device", "VGA"])
try:
    # The getty prints /etc/issue + /run/issue.d/* before "login:"; capture that text.
    vm.expect(r"redrob[-\w]* login: ", timeout=1500)
    R["first_prompt"] = vm.child.before.decode(errors="replace")[-1500:]
    vm.child.sendline("root")
    vm.expect(r"\r?\n# ", timeout=60)
    vm.child.sendline("stty -echo; export PS1='redrob# '")
    vm.expect(r"redrob[^#\r\n]*# ", timeout=20)

    # Timer fires 20 s after boot; wait until a run has fetched the code from the agent.
    for _ in range(24):
        rc, _o = vm.run("grep -q '^  code     [A-Za-z0-9]' /run/issue.d/50-redrob-pairing.issue", timeout=30)
        if rc == 0:
            break
        time.sleep(5)
    R["timer"] = vm.run("systemctl is-active redrob-pairing.timer")[1].splitlines()[-1]
    R["service_result"] = vm.run("systemctl show -p Result -p ExecMainStatus redrob-pairing.service")[1]
    R["journal"] = vm.run("journalctl -u redrob-pairing.service --no-pager -o cat | tail -n 20")[1]
    R["banner"] = vm.run("cat /run/issue.d/50-redrob-pairing.issue")[1]
    R["pairing_json"] = vm.run("cat /run/redrob-pairing/pairing.json")[1]
    R["run_redrob_owner"] = vm.run("stat -c '%U:%G %a' /run/redrob")[1].strip().splitlines()[-1]
    R["issue"] = vm.run("cat /etc/issue")[1]
    R["tty1"] = vm.run("systemctl is-enabled getty@tty1.service ha-cli@tty1.service 2>&1 | tr '\\n' ' '")[1]
    R["device_id"] = vm.run("cat /mnt/data/redrob/identity/device-id")[1].strip().splitlines()[-1]
    R["agent"] = vm.run("systemctl is-active redrob-agent.service")[1].splitlines()[-1]
    R["kconfig_logo"] = vm.run("zcat /proc/config.gz | grep -E '^CONFIG_LOGO(_LINUX_CLUT224)?='")[1]
    R["os_release"] = vm.run("grep -E '^(NAME|VERSION)=' /etc/os-release")[1]

    # Second run of the timer must rewrite the banner (same code until paired).
    time.sleep(35)
    R["banner2_same"] = vm.run("cat /run/issue.d/50-redrob-pairing.issue")[1] == R["banner"]

    vm.monitor(f"screendump {out}/t9-vga.ppm")
    time.sleep(2)
finally:
    vm.kill()

# Derived checks
b = R.get("banner", "")
code = re.search(r"^\s*code\s+(\S+)\s*$", b, re.M)
R["check.code_in_banner"] = bool(code) and "already paired" not in b
try:
    pj = json.loads(R["pairing_json"].strip().splitlines()[-1])
    R["check.json_device_matches"] = pj.get("device") == R.get("device_id")
    R["check.json_code_in_banner"] = pj.get("code", "~") in b
except Exception as e:
    R["check.json_error"] = repr(e)
R["check.qr_in_banner"] = "\u2588" in b or "\u2580" in b or "\u2584" in b
R["check.run_redrob_not_root"] = not R.get("run_redrob_owner", "root:root").startswith("root:")
R["check.banner_on_login_prompt"] = "redrob://pair" in R.get("first_prompt", "") or "pair" in R.get("first_prompt", "").lower()
try:
    px = subprocess.run(["magick", f"{out}/t9-vga.ppm", "-crop", "96x96+0+0", "-format", "%[fx:mean]", "info:"],
                        capture_output=True, text=True, check=True).stdout.strip()
    R["check.logo_tile_mean"] = px
    R["check.logo_visible"] = float(px) > 0.02
except Exception as e:
    R["check.logo_error"] = repr(e)

with open(f"{out}/t9.json", "w") as f:
    json.dump(R, f, indent=2, ensure_ascii=False)
for k, v in R.items():
    if k.startswith("check.") or k in ("timer", "service_result", "tty1", "agent", "kconfig_logo", "banner2_same"):
        print(f"{k}: {v}")
