# Modules

A module is a container that receives a device class from the host after
user approval. Modules install, update and remove independently of the OS
image. Each module directory holds a `module.yaml` manifest.

Manifest fields:

| Field | Meaning |
|---|---|
| `name` | module id, lowercase, unique |
| `version` | semver |
| `capabilities` | device classes the module may receive: `display`, `input`, `storage`, `camera`, `microphone`, `serial`, `accelerator` |
| `devices` | udev match rules that route a hot-plugged device to this module |
| `network` | `none`, or an allowlist of domains |
| `host_access` | `none`, or a list of host paths exposed read-only |
| `approval` | `auto`, `once`, `per-device`, `per-task` |

Modules in this tree:

| Module | Capabilities | Purpose |
|---|---|---|
| `display` | display, input | kiosk (cage + dashboard) or full Wayland desktop; the only module that receives `/dev/dri` and `/dev/input` |
| `usb-broker` | storage | USBGuard policy, approval flow, read-only automount, scoped path exposure to the agent |
| `credential-broker` | none | holds OAuth tokens and API keys outside the sandbox, serves scoped calls, writes the audit log to `/data` |
| `local-inference` | accelerator | llama.cpp router model; backend switch CPU / AI HAT+ 2 / remote GPU over tailnet |
