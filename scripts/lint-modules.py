#!/usr/bin/env python3
"""Lint every modules/*/module.yaml against the schema in modules/README.md.

Exit 0 when all manifests are valid, 1 otherwise. Prints one line per finding.
Only stdlib + PyYAML.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
MODULES = ROOT / "modules"

CAPABILITIES = {"display", "input", "storage", "camera", "microphone", "serial", "accelerator"}
APPROVALS = {"auto", "once", "per-device", "per-task"}
NAME_RE = re.compile(r"^[a-z][a-z0-9-]*$")
SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$")
DOMAIN_RE = re.compile(r"^(\*\.)?[a-z0-9-]+(\.[a-z0-9-]+)+$")
CIDR_RE = re.compile(r"^\d{1,3}(\.\d{1,3}){3}/\d{1,2}$")
REQUIRED = ("name", "version", "description", "capabilities", "devices", "network", "host_access", "approval")


def check(path: Path, seen_names: set[str]) -> list[str]:
    errs: list[str] = []
    rel = path.relative_to(ROOT)
    try:
        doc = yaml.safe_load(path.read_text())
    except yaml.YAMLError as e:  # noqa: BLE001
        return [f"{rel}: invalid YAML: {e}"]
    if not isinstance(doc, dict):
        return [f"{rel}: top level must be a mapping"]

    for key in REQUIRED:
        if key not in doc:
            errs.append(f"{rel}: missing required field `{key}`")

    name = doc.get("name")
    if isinstance(name, str):
        if not NAME_RE.match(name):
            errs.append(f"{rel}: name `{name}` must be lowercase [a-z0-9-]")
        if name != path.parent.name:
            errs.append(f"{rel}: name `{name}` must equal directory `{path.parent.name}`")
        if name in seen_names:
            errs.append(f"{rel}: duplicate module name `{name}`")
        seen_names.add(name)

    version = doc.get("version")
    if version is not None and not (isinstance(version, str) and SEMVER_RE.match(version)):
        errs.append(f"{rel}: version `{version}` is not semver (quote it in YAML)")

    desc = doc.get("description")
    if desc is not None and (not isinstance(desc, str) or len(desc.strip()) < 10):
        errs.append(f"{rel}: description must be a sentence")

    caps = doc.get("capabilities")
    if caps is not None:
        if not isinstance(caps, list):
            errs.append(f"{rel}: capabilities must be a list")
        else:
            for c in caps:
                if c not in CAPABILITIES:
                    errs.append(f"{rel}: unknown capability `{c}`")

    devices = doc.get("devices")
    if devices is not None and not isinstance(devices, list):
        errs.append(f"{rel}: devices must be a list of udev match rules")
    elif isinstance(devices, list):
        for d in devices:
            if not isinstance(d, dict) or not d:
                errs.append(f"{rel}: each device rule must be a non-empty mapping")

    net = doc.get("network")
    if net is not None and net != "none":
        if not isinstance(net, list) or not net:
            errs.append(f"{rel}: network must be `none` or a non-empty domain list")
        else:
            for d in net:
                if not (isinstance(d, str) and (DOMAIN_RE.match(d) or CIDR_RE.match(d))):
                    errs.append(f"{rel}: network entry `{d}` is not a domain, *.domain or CIDR")

    host = doc.get("host_access")
    if host is not None and host != "none":
        if not isinstance(host, list) or not host:
            errs.append(f"{rel}: host_access must be `none` or a non-empty path list")
        else:
            for p in host:
                if not (isinstance(p, str) and p.startswith("/")):
                    errs.append(f"{rel}: host_access entry `{p}` must be an absolute path")

    approval = doc.get("approval")
    if approval is not None and approval not in APPROVALS:
        errs.append(f"{rel}: approval `{approval}` not in {sorted(APPROVALS)}")

    for key in ("secrets_store", "audit_log", "models_dir"):
        v = doc.get(key)
        if v is not None and not (isinstance(v, str) and v.startswith("/data/")):
            errs.append(f"{rel}: {key} must live under /data/ (persistent partition)")

    return errs


def main() -> int:
    manifests = sorted(MODULES.glob("*/module.yaml"))
    if not manifests:
        print("no module manifests found", file=sys.stderr)
        return 1
    seen: set[str] = set()
    errs: list[str] = []
    for m in manifests:
        errs.extend(check(m, seen))
    for e in errs:
        print(e)
    print(f"{len(manifests)} manifests, {len(errs)} findings")
    return 1 if errs else 0


if __name__ == "__main__":
    sys.exit(main())
