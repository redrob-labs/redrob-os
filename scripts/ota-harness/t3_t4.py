#!/usr/bin/env python3
"""T3: corrupted / truncated bundle is refused, grubenv untouched, A still boots.
T4: power cut while writing /mnt/data -> next boot: fsck clean-up, written files intact."""
import shutil, sys, os, time
sys.path.insert(0, os.path.dirname(__file__))
from vm import VM, W, fetch

disk = f"{W}/t3.img"
shutil.copyfile(f"{W}/a.img", disk)
R = {}

def grubenv(vm):
    return vm.check("grep -v '^#' /mnt/boot/EFI/BOOT/grubenv | grep -v MACHINE_ID | tr '\\n' ' '")

vm = VM(disk, "t3-boot1")
try:
    vm.login()
    R["grubenv.before"] = grubenv(vm)
    for name in ("broken-payload", "broken-truncated"):
        b = fetch(vm, name)
        rc, out = vm.run(f"set -o pipefail; rauc install {b} 2>&1 | tr -d '\\r' | grep -iE 'error|fail|invalid|signature|verif|succeeded' | tail -3", timeout=1800)
        R[f"{name}.rc"] = rc
        R[f"{name}.msg"] = out.replace("\n", " | ")
        R[f"{name}.grubenv"] = grubenv(vm)
    R["slotB.untouched"] = vm.check("rauc status --detailed 2>/dev/null | grep -A8 'rootfs.1' | grep -iE 'status|installed' | tr '\\n' ' ' || true")

    # T4: sustained writes to /mnt/data, then power cut
    vm.check("mkdir -p /mnt/data/redrob/t4 && cd /mnt/data/redrob/t4 && "
             "for i in $(seq 1 20); do head -c 1048576 /dev/urandom > f$i; sha256sum f$i >> ../t4.sha; sync; done && cd /")
    R["t4.synced-files"] = vm.check("wc -l < /mnt/data/redrob/t4.sha")
    vm.child.sendline("(cd /mnt/data/redrob/t4 && i=100; while :; do head -c 4194304 /dev/urandom > w$i; i=$((i+1)); done) &")
    vm.expect(r"redrob# ", timeout=20)
    time.sleep(8)
    rc, out = vm.run("ls /mnt/data/redrob/t4 | wc -l")
    R["t4.files-at-cut"] = out.strip()
    vm.kill()
finally:
    if vm.proc.poll() is None:
        vm.kill()

vm = VM(disk, "t3-boot2")
try:
    vm.login(timeout=1200)
    R["after.cmdline"] = vm.check("grep -o 'rauc.slot=[AB]' /proc/cmdline")
    R["after.grubenv"] = grubenv(vm)
    R["after.data-mount"] = vm.check("findmnt -no SOURCE,FSTYPE,OPTIONS /mnt/data")
    R["after.fsck"] = vm.check("journalctl -b --no-pager -o cat | grep -iE 'systemd-fsck|e2fsck|recovering journal|redrob-data' | head -6 | tr '\\n' ' '")
    rc, out = vm.run("cd /mnt/data/redrob/t4 && sha256sum -c ../t4.sha 2>&1 | grep -c ': OK'")
    R["after.synced-files-ok"] = f"{out.strip()}/20"
    R["after.unsynced-files"] = vm.check("ls /mnt/data/redrob/t4 | grep -c '^w' || true")
    R["after.identity"] = vm.check("ls /mnt/data/redrob/identity | tr '\\n' ' '")
    R["after.agent"] = vm.check("systemctl is-active redrob-agent")
    R["after.failed-units"] = vm.check("systemctl list-units --state=failed --no-legend | tr '\\n' ' '")
finally:
    vm.kill()

for k, v in R.items():
    print(f"{k}: {v}")
