#!/usr/bin/env python3
"""T1: rauc install dev1 bundle -> reboot -> boots slot B -> mark-good resets TRY."""
import shutil, sys, os, time
sys.path.insert(0, os.path.dirname(__file__))
from vm import VM, W, fetch

disk = f"{W}/t1.img"
shutil.copyfile(f"{W}/a.img", disk)
R = {}

def grubenv(vm):
    return vm.check("grep -v '^#' /mnt/boot/EFI/BOOT/grubenv | grep -v MACHINE_ID | tr '\\n' ' '")

vm = VM(disk, "t1-boot1")
try:
    vm.login()
    R["before.os"] = vm.check("grep ^VERSION= /etc/os-release")
    R["before.rauc"] = vm.check("rauc status | grep -E 'Booted|boot status' | tr '\\n' ' '")
    R["before.grubenv"] = grubenv(vm)
    t = time.time()
    b = fetch(vm, "update"); R["fetch.secs"] = int(time.time() - t); t = time.time()
    rc, out = vm.run(f"set -o pipefail; rauc install {b} 2>&1 | tail -5", timeout=2400)
    R["install.rc"] = rc
    R["install.secs"] = int(time.time() - t)
    R["install.tail"] = out.replace("\r", "")
    R["after-install.grubenv"] = grubenv(vm)
    R["after-install.rauc"] = vm.check("rauc status | grep -E 'Booted|boot status|Installed' | tr '\\n' ' '")
    R["reboot1"] = vm.reboot()
finally:
    if vm.proc.poll() is None:
        vm.kill()

vm = VM(disk, "t1-boot2")
try:
    vm.login()
    R["B.cmdline"] = vm.check("grep -o 'rauc.slot=[AB]' /proc/cmdline")
    R["prev-shutdown.stalls"] = vm.check("journalctl -b -1 --no-pager -o short-monotonic | grep -iE 'timed out|Killing process|State .stop-sigterm' | head -5 | tr -d '\\r' | tr '\\n' ' ' || true")
    R["B.os"] = vm.check("grep ^VERSION= /etc/os-release")
    R["B.rauc.booted"] = vm.check("rauc status | grep Booted")
    # mark-good waits for the agent /health; give it time under TCG
    rc, out = vm.run("for i in $(seq 1 60); do s=$(systemctl is-active redrob-mark-good); [ \"$s\" = active ] && break; [ \"$s\" = failed ] && break; sleep 5; done; echo $s", timeout=400)
    R["B.mark-good"] = out.strip().splitlines()[-1]
    R["B.mark-good.log"] = vm.check("journalctl -u redrob-mark-good --no-pager -o cat | tail -3 | tr '\\n' ' '")
    R["B.grubenv"] = grubenv(vm)
    R["B.rauc.status"] = vm.check("rauc status | grep -E 'boot status' | tr '\\n' ' '")
    R["B.failed-units"] = vm.check("systemctl list-units --state=failed --no-legend | tr '\\n' ' '")
    R["reboot2"] = vm.reboot()
finally:
    if vm.proc.poll() is None:
        vm.kill()

# third boot: still B, TRY stays bounded (mark-good runs again)
vm = VM(disk, "t1-boot3")
try:
    vm.login()
    R["B2.cmdline"] = vm.check("grep -o 'rauc.slot=[AB]' /proc/cmdline")
    R["B2.grubenv.at-login"] = grubenv(vm)
    vm.run("for i in $(seq 1 60); do s=$(systemctl is-active redrob-mark-good); [ \"$s\" = active ] && break; [ \"$s\" = failed ] && break; sleep 5; done", timeout=400)
    R["B2.grubenv.after-mark-good"] = grubenv(vm)
finally:
    vm.kill()

for k, v in R.items():
    print(f"{k}: {v}")
