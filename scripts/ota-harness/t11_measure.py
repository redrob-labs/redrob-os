#!/usr/bin/env python3
"""T11: Stage 8 final measurement on a fresh dev4 image.

Usage: t11_measure.py <raw-disk.img> <outdir>

Confirms, after first boot of the dev4 image (now carrying local-inference +
llama-cpp):
  - systemd reaches running/degraded and lists any failed units
  - every redrob unit is in its expected state (agent/broker/usb-broker/
    pairing.timer/local-inference active; firstboot+mark-good oneshots
    succeeded; display skipped in headless default)
  - local-inference is up and WAITING for a model (none is provisioned on a
    fresh image -> no llama-server, low memory): the shipped idle state
  - idle memory used (target <= 1024 MiB) after a settle
  - boot -> agent-ready latency (redrob-agent active-enter, monotonic since
    boot) and whether the gateway port is listening
Writes <outdir>/t11.json.
"""
import json, os, re, sys, time

sys.path.insert(0, os.path.dirname(__file__))
from vm import VM

disk, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
R = {}
vm = VM(disk, "t11")
try:
    vm.login(timeout=1500)

    # Let the system settle. is-system-running flips to "degraded" early (the
    # QEMU-only haos-swapfile/growfs units fail within seconds), so do not stop
    # there: also wait for the long-pole daemons (brokers wait on
    # network-online.target) to leave "activating".
    state = ""
    for _ in range(60):
        _rc, out_s = vm.run("systemctl is-system-running", timeout=30)
        state = out_s.splitlines()[-1].strip()
        if state in ("running", "degraded"):
            break
        time.sleep(5)
    for _ in range(36):  # up to ~3 min for brokers to finish starting
        _rc, st = vm.run(
            "systemctl is-active redrob-broker.service redrob-usb-broker.service "
            "redrob-local-inference.service redrob-agent.service | tr '\\n' ' '",
            timeout=30,
        )
        if "activating" not in st:
            break
        time.sleep(5)
    R["system_state"] = state
    R["failed_units"] = vm.run("systemctl --failed --no-legend --plain | cat")[1]

    units = {
        "agent": "redrob-agent.service",
        "broker": "redrob-broker.service",
        "usb_broker": "redrob-usb-broker.service",
        "local_inference": "redrob-local-inference.service",
        "pairing_timer": "redrob-pairing.timer",
    }
    for key, unit in units.items():
        R[f"active.{key}"] = vm.run(f"systemctl is-active {unit}")[1].splitlines()[-1].strip()
    # Oneshots: success is inactive + Result=success / ExecMainStatus=0.
    R["firstboot_result"] = vm.run("systemctl show -p Result -p ExecMainStatus redrob-firstboot.service")[1]
    R["markgood_result"] = vm.run("systemctl show -p Result redrob-mark-good.service 2>&1 | tail -1")[1]
    # display is condition-gated in headless default (no /mnt/data/redrob/display/mode=kiosk).
    R["display_mode_active"] = vm.run("systemctl is-active redrob-display-mode.service 2>&1 | tail -1")[1].strip()

    # local-inference: unit active, supervisor polling, NO llama-server yet
    # (no model on a fresh image). That is the correct shipped idle state.
    R["li_journal"] = vm.run("journalctl -u redrob-local-inference.service --no-pager -o cat | tail -n 8")[1]
    R["li_llama_running"] = vm.run("pgrep -a llama-server | cat")[1]
    R["li_models_dir"] = vm.run("ls -la /mnt/data/redrob/models | cat")[1]
    R["li_port_8081"] = vm.run("ss -ltn 2>/dev/null | grep ':8081' | cat")[1]
    R["li_config"] = vm.run("cat /etc/redrob/local-inference.toml | grep -E 'listen_|model_alias|models_dir' | cat")[1]
    R["li_user"] = vm.run("id redrob-infer 2>&1 | tail -1")[1]
    R["llama_server_bin"] = vm.run("ls -la /usr/bin/llama-server /usr/bin/redrob-local-inference | cat")[1]

    # Idle memory: settle, then read used MiB from free -m.
    time.sleep(30)
    R["free"] = vm.run("free -m")[1]
    m = re.search(r"^Mem:\s+(\d+)\s+(\d+)\s+(\d+)", R["free"], re.M)
    if m:
        R["mem_total_mib"] = int(m.group(1))
        R["mem_used_mib"] = int(m.group(2))

    # Boot -> agent-ready latency (monotonic since boot, in us).
    R["agent_active_enter_us"] = vm.run(
        "systemctl show -p ActiveEnterTimestampMonotonic --value redrob-agent.service"
    )[1].splitlines()[-1].strip()
    R["gateway_port"] = vm.run("ss -ltn 2>/dev/null | grep ':42617' | cat")[1]
    R["boot_time"] = vm.run("systemd-analyze 2>&1 | head -1 | cat")[1]
    R["blame_top"] = vm.run("systemd-analyze blame --no-pager 2>/dev/null | head -8 | cat")[1]
    R["os_release"] = vm.run("grep -E '^(NAME|VERSION)=' /etc/os-release")[1]
finally:
    vm.kill()

# Derived verdicts
def active(k):
    return R.get(f"active.{k}", "") == "active"

R["check.system_ok"] = R.get("system_state") in ("running", "degraded")
R["check.no_failed_units"] = R.get("failed_units", "x").strip() == ""
R["check.agent_active"] = active("agent")
R["check.broker_active"] = active("broker")
R["check.usb_broker_active"] = active("usb_broker")
R["check.local_inference_active"] = active("local_inference")
R["check.pairing_timer_active"] = active("pairing_timer")
R["check.firstboot_success"] = "ExecMainStatus=0" in R.get("firstboot_result", "")
R["check.li_waiting_no_llama"] = R.get("li_llama_running", "x").strip() == ""
R["check.li_binaries_installed"] = "/usr/bin/llama-server" in R.get("llama_server_bin", "") and \
    "/usr/bin/redrob-local-inference" in R.get("llama_server_bin", "")
R["check.li_user_exists"] = "redrob-infer" in R.get("li_user", "")
R["check.idle_mem_under_1gib"] = R.get("mem_used_mib", 99999) <= 1024
try:
    R["agent_ready_s"] = round(int(R["agent_active_enter_us"]) / 1_000_000, 1)
except Exception:
    R["agent_ready_s"] = None

with open(f"{out}/t11.json", "w") as f:
    json.dump(R, f, indent=2, ensure_ascii=False)

for k in sorted(R):
    if k.startswith("check.") or k in ("system_state", "mem_used_mib", "mem_total_mib",
                                        "agent_ready_s", "boot_time", "failed_units"):
        print(f"{k}: {R[k]}")
