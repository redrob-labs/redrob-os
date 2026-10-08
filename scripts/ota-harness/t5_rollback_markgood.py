#!/usr/bin/env python3
"""T5a: t1.img has B_TRY=3 (agent never healthy on B) -> GRUB must fall back to slot A.
T5b: t2.img has B installed (B_OK=1) -> boots B, agent healthy (-cpu max), mark-good -> B_TRY=0."""
import sys, os, time
sys.path.insert(0, os.path.dirname(__file__))
from vm import VM, W
R = {}
def grubenv(vm):
    return vm.check("grep -v '^#' /mnt/boot/EFI/BOOT/grubenv | grep -v MACHINE_ID | tr '\\n' ' '")

vm = VM(f"{W}/t1.img", "t5a")
try:
    vm.login(timeout=1500)
    R["5a.cmdline"] = vm.check("grep -o 'rauc.slot=[AB]' /proc/cmdline")
    R["5a.os"] = vm.check("grep ^VERSION= /etc/os-release")
    R["5a.grubenv"] = grubenv(vm)
    R["5a.rauc"] = vm.check("rauc status 2>/dev/null | grep -E 'Booted|boot status' | tr '\\n' ' '")
    R["5a.cpu"] = vm.check("grep -m1 'model name' /proc/cpuinfo")
    rc, out = vm.run("for i in $(seq 1 90); do curl -sf -m 3 http://127.0.0.1:42617/health >/dev/null && { echo healthy after ~$((i*5))s; break; }; sleep 5; done; systemctl show redrob-agent -p NRestarts", timeout=600)
    R["5a.agent"] = out.replace("\n", " ")
finally:
    vm.kill()

vm = VM(f"{W}/t2.img", "t5b")
try:
    vm.login(timeout=1500)
    R["5b.cmdline"] = vm.check("grep -o 'rauc.slot=[AB]' /proc/cmdline")
    R["5b.os"] = vm.check("grep ^VERSION= /etc/os-release")
    R["5b.grubenv.at-login"] = grubenv(vm)
    rc, out = vm.run("for i in $(seq 1 90); do s=$(systemctl is-active redrob-mark-good); [ \"$s\" = active ] && break; [ \"$s\" = failed ] && break; sleep 5; done; echo $s", timeout=600)
    R["5b.mark-good"] = out.strip().splitlines()[-1]
    R["5b.mark-good.log"] = vm.check("journalctl -u redrob-mark-good --no-pager -o short-monotonic | tr -d '\\r' | grep -v 'Consumed' | tail -4 | tr '\\n' ' '")
    R["5b.grubenv.after"] = grubenv(vm)
    R["5b.rauc"] = vm.check("rauc status 2>/dev/null | grep -E 'Booted|boot status' | tr '\\n' ' '")
    R["5b.agent"] = vm.check("systemctl show redrob-agent -p NRestarts -p ActiveState | tr '\\n' ' '; curl -sf -m 3 http://127.0.0.1:42617/health; echo")
    R["5b.failed-units"] = vm.check("systemctl list-units --state=failed --no-legend | tr '\\n' ' '")
    R["5b.reboot"] = vm.reboot(timeout=900)
finally:
    if vm.proc.poll() is None: vm.kill()

for k, v in R.items():
    print(f"{k}: {v}")
