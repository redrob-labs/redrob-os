#!/usr/bin/env python3
"""Drive a Redrob OS x86-64 image under QEMU TCG over its serial console.

Usage (library): see scenario scripts. The guest root login has no password,
the serial getty is enabled by `console=ttyS0` in the ESP cmdline.txt.
"""
import os, socket, subprocess, sys, time
import pexpect
from pexpect import fdpexpect

QEMU = os.path.expanduser("~/.local/qemu/bin/qemu-system-x86_64")
BIOS = os.path.expanduser("~/.local/qemu/share/qemu/edk2-x86_64-code.fd")
VARS = os.path.expanduser("~/.local/qemu/share/qemu/edk2-i386-vars.fd")
W = os.path.dirname(os.path.abspath(__file__))
PROMPT = r"redrob[^#\r\n]*# "


class VM:
    def __init__(self, disk, name, http_port=None, extra=()):
        self.disk, self.name = disk, name
        self.serial_path = f"{W}/{name}.serial.sock"
        self.mon_path = f"{W}/{name}.mon.sock"
        self.log = open(f"{W}/{name}.serial.log", "ab")
        for p in (self.serial_path, self.mon_path):
            if os.path.exists(p):
                os.unlink(p)
        self.vars = f"{W}/{name}.vars.fd"
        if not os.path.exists(self.vars):
            import shutil
            shutil.copyfile(VARS, self.vars)
        cmd = [QEMU, "-machine", "q35,accel=tcg", "-cpu", "max", "-smp", "4", "-m", "2048",
               "-drive", f"if=pflash,format=raw,readonly=on,file={BIOS}",
               "-drive", f"if=pflash,format=raw,file={self.vars}",
               "-display", "none",
               "-drive", f"file={disk},format=raw,if=virtio",
               "-nic", "user,model=virtio-net-pci",
               "-chardev", f"socket,id=ser,path={self.serial_path},server=on,wait=off",
               "-serial", "chardev:ser",
               "-monitor", f"unix:{self.mon_path},server,nowait",
               "-no-reboot"] + list(extra)
        self.proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=self.log)
        for _ in range(100):
            if os.path.exists(self.serial_path):
                break
            time.sleep(0.1)
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(self.serial_path)
        self.child = fdpexpect.fdspawn(self.sock.fileno(), timeout=600, logfile=self.log)

    # -- console -------------------------------------------------------
    def expect(self, pat, timeout=600):
        return self.child.expect(pat, timeout=timeout)

    def login(self, timeout=900):
        """Wait for the serial getty and log in as root."""
        self.expect(r"redrob[-\w]* login: ", timeout=timeout)
        self.child.sendline("root")
        self.expect(r"\r?\n# ", timeout=60)
        self.child.sendline("stty -echo; export PS1='redrob# '")
        self.expect(PROMPT, timeout=20)

    def run(self, cmd, timeout=600):
        """Run a shell command, return (rc, output)."""
        marker = "__RC_%d__" % int(time.time() * 1000)
        self.child.sendline(f"{cmd}; echo {marker}$?")
        self.expect(marker + r"(\d+)", timeout=timeout)
        rc = int(self.child.match.group(1))
        out = self.child.before.decode(errors="replace")
        self.expect(PROMPT, timeout=20)
        return rc, out.strip()

    def check(self, cmd, timeout=600):
        rc, out = self.run(cmd, timeout)
        if rc != 0:
            raise RuntimeError(f"{cmd!r} rc={rc}\n{out}")
        return out

    # -- lifecycle -----------------------------------------------------
    def monitor(self, cmd):
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            s.connect(self.mon_path)
        except OSError:
            self.kill(); return
        s.settimeout(5)
        try:
            s.recv(4096)
        except socket.timeout:
            pass
        s.sendall((cmd + "\n").encode())
        time.sleep(0.5)
        s.close()

    def kill(self):
        """Simulate a power cut: SIGKILL qemu, no guest shutdown."""
        self.proc.kill()
        self.proc.wait()
        self.log.flush()

    def wait_exit(self, timeout=300):
        self.proc.wait(timeout=timeout)
        self.log.flush()

    def reboot(self, timeout=1500):
        """Clean reboot; returns 'clean' if qemu exited (-no-reboot), else a
        diagnostic string and a hard reset (monitor quit) -- a power cycle."""
        self.child.sendline("systemctl reboot --no-block; echo SENT")
        try:
            self.expect("SENT", timeout=30)
        except Exception:
            pass
        t0 = time.time()
        pokes = 0
        while time.time() - t0 < timeout:
            try:
                self.wait_exit(30)
                return f"clean ({int(time.time()-t0)}s, {pokes} console pokes)"
            except subprocess.TimeoutExpired:
                # Finding: shutdown stalls until the serial console sees input.
                try:
                    self.child.sendline("")
                except Exception:
                    pass
                pokes += 1
        try:
            self.wait_exit(1)
            return f"clean ({int(time.time()-t0)}s, {pokes} console pokes)"
        except subprocess.TimeoutExpired:
            diag = ""
            try:
                rc, diag = self.run("systemctl list-jobs --no-legend | head; systemctl is-system-running", timeout=30)
            except Exception as e:
                diag = f"console unresponsive: {type(e).__name__}"
            self.monitor("quit")
            try:
                self.wait_exit(30)
            except subprocess.TimeoutExpired:
                self.kill()
            return "HUNG; hard reset. " + diag.replace("\n", " | ")


def serve(directory, port):
    """HTTP server on loopback; guest reaches it as 10.0.2.2:port (slirp)."""
    return subprocess.Popen([sys.executable, "-m", "http.server", str(port),
                             "--bind", "127.0.0.1", "--directory", directory],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


BUNDLE_DIR = "/mnt/data/redrob/ota"


def fetch(vm, name, port=18090):
    """Download a bundle into /mnt/data (what the agent will do); return guest path."""
    vm.check(f"mkdir -p {BUNDLE_DIR} && curl -sf -o {BUNDLE_DIR}/{name}.raucb http://10.0.2.2:{port}/{name}.raucb && sync", timeout=1200)
    return f"{BUNDLE_DIR}/{name}.raucb"
