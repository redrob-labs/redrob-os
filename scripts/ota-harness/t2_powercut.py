#!/usr/bin/env python3
"""T2: kill qemu while rauc is writing slot B -> next boot must come up on A."""
import shutil, sys, os, time
sys.path.insert(0, os.path.dirname(__file__))
from vm import VM, W, fetch

disk = f"{W}/t2.img"
shutil.copyfile(f"{W}/a.img", disk)
R = {}

def grubenv(vm):
    return vm.check("grep -v '^#' /mnt/boot/EFI/BOOT/grubenv | grep -v MACHINE_ID | tr '\\n' ' '")

vm = VM(disk, "t2-boot1")
try:
    vm.login()
    R["before.grubenv"] = grubenv(vm)
    # background install; poll progress and cut power while the rootfs slot is being written
    b = fetch(vm, "update")
    vm.child.sendline(f"rauc install {b} > /tmp/inst.log 2>&1 &")
    vm.expect(r"redrob# ", timeout=20)
    cut_at = None
    for i in range(240):
        time.sleep(5)
        rc, out = vm.run("tail -c 300 /tmp/inst.log | tr -d '\\r' | tail -2")
        if "Copying image" in out or "Updating slot" in out:
            time.sleep(10)   # be inside the slot write, not at its first byte
            rc, out = vm.run("tail -c 300 /tmp/inst.log | tr -d '\\r' | tail -2")
            cut_at = out.replace("\n", " | ")
            break
        if "succeeded" in out or "failed" in out:
            cut_at = "TOO LATE: " + out
            break
    R["cut-at"] = cut_at
    vm.kill()   # power cut
finally:
    if vm.proc.poll() is None:
        vm.kill()

vm = VM(disk, "t2-boot2")
try:
    vm.login(timeout=1200)
    R["after.cmdline"] = vm.check("grep -o 'rauc.slot=[AB]' /proc/cmdline")
    R["after.os"] = vm.check("grep ^VERSION= /etc/os-release")
    R["after.grubenv"] = grubenv(vm)
    R["after.rauc"] = vm.check("rauc status | grep -E 'Booted|boot status' | tr '\\n' ' '")
    R["after.failed-units"] = vm.check("systemctl list-units --state=failed --no-legend | tr '\\n' ' '")
    R["data.fsck"] = vm.check("journalctl -b --no-pager -o cat | grep -iE 'fsck|e2fsck|recovering journal|clean,' | head -5 | tr '\\n' ' '")
    # a second, complete install must still work after the interrupted one
    rc, out = vm.run(f"set -o pipefail; rauc install {fetch(vm, 'update')} 2>&1 | tail -2", timeout=2400)
    R["reinstall.rc"] = rc
    R["reinstall.tail"] = out.replace("\r", "")
    R["reinstall.grubenv"] = grubenv(vm)
finally:
    vm.kill()

for k, v in R.items():
    print(f"{k}: {v}")
