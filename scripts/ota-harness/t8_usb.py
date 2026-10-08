#!/usr/bin/env python3
"""T8: USB broker on the real guest kernel. Guest = t2.img (slot B, dev1, agent healthy).
The broker binary (static musl) is fetched into /mnt/data and run as root; devices are
hot-plugged through the QEMU monitor (xhci)."""
import sys, os, time, json
sys.path.insert(0, os.path.dirname(__file__))
from vm import VM, W

STICK = f"{W}/stick.img"
R = {}
vm = VM(f"{W}/t2.img", "t8", extra=["-device", "qemu-xhci,id=xhci",
                                     "-drive", f"if=none,id=stick,file={STICK},format=raw"])
B = "/mnt/data/redrob/usb-broker-test"
SOCK = "/run/redrob/usb.sock"
def api(path, method="GET", body=None):
    data = f" -d '{json.dumps(body)}' -H 'content-type: application/json'" if body is not None else ""
    rc, out = vm.run(f"curl -s -X {method}{data} --unix-socket {SOCK} http://usb{path}", timeout=60)
    try:
        return json.loads(out.strip().splitlines()[-1])
    except Exception:
        return {"raw": out}
def sysfs(name, attr):
    return vm.run(f"cat /sys/bus/usb/devices/{name}/{attr} 2>&1")[1].strip().splitlines()[-1]
try:
    vm.login(timeout=1500)
    vm.check(f"mkdir -p {B} && curl -sf -o {B}/redrob-usb-broker http://10.0.2.2:18090/redrob-usb-broker && chmod 755 {B}/redrob-usb-broker", timeout=600)
    R["kernel"] = vm.check("uname -r; ls /sys/bus/usb/devices/ | tr '\\n' ' '")
    R["authorized_default.before"] = vm.check("cat /sys/bus/usb/devices/usb*/authorized_default | tr '\\n' ' '")
    vm.child.sendline(f"RUST_LOG=info {B}/redrob-usb-broker --policy {B}/policy.json --audit-dir {B}/audit --media-dir /run/media/redrob > {B}/broker.log 2>&1 &")
    vm.expect(r"redrob# ", timeout=20)
    time.sleep(3)
    R["authorized_default.after"] = vm.check("cat /sys/bus/usb/devices/usb*/authorized_default | tr '\\n' ' '")
    R["health"] = api("/v1/health")

    # 1. keyboard hot-plug -> pending, no input device; approve -> authorized, hid bound
    vm.monitor("device_add usb-kbd,bus=xhci.0,id=kbd1")
    time.sleep(6)
    devs = api("/v1/devices")["devices"]
    kbd = next((k for k, v in devs.items() if v["device"]["product_id"] == "0001"), None)
    R["kbd.sysname"] = kbd
    if kbd:
        R["kbd.verdict"] = devs[kbd]["verdict"], devs[kbd]["reason"]
        R["kbd.authorized.pending"] = sysfs(kbd, "authorized")
        R["kbd.hid.pending"] = vm.run(f"ls /sys/bus/usb/devices/{kbd}/{kbd}:1.0/ 2>/dev/null | grep -c input || true")[1].strip().splitlines()[-1]
        R["kbd.approve"] = api(f"/v1/devices/{kbd}/approve", "POST", {"remember": True})
        time.sleep(4)
        R["kbd.authorized.after"] = sysfs(kbd, "authorized")
        R["kbd.hid.after"] = vm.run(f"ls /sys/bus/usb/devices/{kbd}/{kbd}:1.0/ 2>/dev/null | grep -c input || true")[1].strip().splitlines()[-1]
        R["kbd.input"] = vm.run("ls /dev/input/ | tr '\\n' ' '")[1].strip().splitlines()[-1]

    # 2. storage hot-plug -> pending (no /dev/sd*); approve -> sda appears, mounted ro; write fails
    vm.monitor("device_add usb-storage,bus=xhci.0,drive=stick,id=stick1")
    time.sleep(8)
    devs = api("/v1/devices")["devices"]
    stk = next((k for k, v in devs.items() if any(i["class"] == 8 for i in v["device"]["interfaces"])), None)
    R["stick.sysname"] = stk
    if stk:
        R["stick.verdict"] = devs[stk]["verdict"], devs[stk]["reason"]
        R["stick.blockdev.pending"] = vm.run("ls /dev/sd* 2>&1 | tr '\\n' ' '")[1].strip().splitlines()[-1]
        R["stick.approve"] = api(f"/v1/devices/{stk}/approve", "POST", {"remember": False})
        time.sleep(15)
        R["stick.blockdev.after"] = vm.run("ls /dev/sd* 2>&1 | tr '\\n' ' '")[1].strip().splitlines()[-1]
        R["stick.mounts"] = api("/v1/devices")["devices"].get(stk, {}).get("mounts")
        R["stick.findmnt"] = vm.run("findmnt -rno TARGET,SOURCE,OPTIONS /run/media/redrob/REDROBSTK 2>&1")[1].strip().splitlines()[-1]
        R["stick.read"] = vm.run("cat /run/media/redrob/REDROBSTK/hello.txt 2>&1")[1].strip().splitlines()[-1]
        R["stick.write"] = vm.run("echo x > /run/media/redrob/REDROBSTK/evil.txt 2>&1; echo rc=$?")[1].strip().splitlines()[-1]
        # 3. unplug -> unmounted, record gone
        vm.monitor("device_del stick1")
        time.sleep(6)
        R["stick.after-unplug.mount"] = vm.run("findmnt -rno TARGET /run/media/redrob/REDROBSTK 2>&1 || echo gone")[1].strip().splitlines()[-1]
        R["stick.after-unplug.record"] = stk in api("/v1/devices")["devices"]

    # 4. replug the keyboard: remembered -> allowed immediately
    vm.monitor("device_del kbd1")
    time.sleep(4)
    vm.monitor("device_add usb-kbd,bus=xhci.0,id=kbd2")
    time.sleep(6)
    devs = api("/v1/devices")["devices"]
    kbd2 = next((k for k, v in devs.items() if v["device"]["product_id"] == "0001"), None)
    R["kbd2.verdict"] = devs[kbd2]["verdict"] if kbd2 else None
    R["kbd2.authorized"] = sysfs(kbd2, "authorized") if kbd2 else None
    R["policy.json"] = vm.run(f"cat {B}/policy.json | tr -d '\\n '")[1].strip().splitlines()[-1]
    R["audit"] = vm.run(f"cut -c1-160 {B}/audit/usb-*.jsonl | sed 's/\"ts\":\"[^\"]*\",//'")[1]
    R["broker.log.tail"] = vm.run(f"tail -5 {B}/broker.log | cut -c1-160")[1]
finally:
    vm.kill()
for k, v in R.items():
    print(f"{k}: {v}")
