# Remote access

How a phone (or any client) reaches a Redrob OS device's dashboard and agent.

There are two paths. Pick by where the client is.

| Where the client is | Path | Needs a server? |
|---|---|---|
| Same Wi‑Fi / LAN as the device | **LAN‑direct** | no |
| Anywhere else (mobile data, another network) | **Relay** | yes — a relay you run |

## LAN‑direct (default, on same network)

The device gateway (dashboard + pairing) listens on all interfaces at
`:42617`. Every route is gated by the one‑time **pairing token**, so only a
paired client gets in.

1. Put the phone on the **same Wi‑Fi** as the device.
2. Scan the pairing QR on the device screen (or read the code over serial).
   It encodes `redrob://pair?device=<id>&host=<device-ip>:42617&code=<code>&fp=<sha256>`.
3. The phone pairs with that code and opens the device dashboard at
   `https://<device-ip>:42617`, pinning the certificate whose SHA‑256
   fingerprint is `fp`.

Encryption: the listener serves **HTTPS**. On first start the agent
self‑provisions a certificate under its data dir (`gateway-tls/`) and reuses it
on every later start, so the pin survives reboots. The certificate is
self‑signed (per‑device CA), so clients must pin `fp` rather than rely on
public CAs; a browser opened at the address shows a certificate warning.
`GET /admin/paircode` (loopback + admin token) reports the same value as
`tls_fingerprint`. Loopback callers (health check, pairing banner) may still
use plain HTTP on the same port.

| `[gateway.tls]` | Behaviour |
|---|---|
| `enabled = true` (image default), no paths | HTTPS, self‑provisioned certificate |
| `enabled = true`, `cert_path` + `key_path` | HTTPS with your own certificate |
| `enabled = false` (or no `[gateway.tls]`) | plain HTTP — trusted LANs only |

## Relay (from anywhere)

Off‑LAN, the phone cannot reach the device's private address. A **relay** is a
public rendezvous the device dials out to; the phone then reaches the device
*through* the relay by an opaque node id. The relay is a **blind forwarder** —
the client↔device mTLS terminates on the device, never on the relay, so the
relay never sees your traffic in the clear.

Redrob does not operate a public relay yet, so you run your own. It is one
binary (`zerorelay`) on any host with a public address.

### 1. Run a relay on a public host

```sh
# On a VPS / box with a public IP or DNS name:
zerorelay \
  --bind 0.0.0.0:8443 \
  --tls-san relay.example.com \      # your relay's public hostname
  --relay-token "$(head -c24 /dev/urandom | base64)"   # admission token; keep it
```

- No `--tls-cert`/`--tls-key`: the relay **self‑provisions** its own TLS cert on
  first run (no openssl needed) and reuses it after.
- The `--relay-token` it prints/accepts is the admission credential devices must
  present. Save it.
- Open inbound TCP `8443` on the host's firewall / security group.

### 2. Point the device at it

On the device dashboard, open **Network → Relay** and set:

| Field | Value |
|---|---|
| `enabled` | `true` |
| `url` | `relay.example.com:8443` |
| `token` | the `--relay-token` from step 1 (stored encrypted) |
| `tofu` | `true` — trust the relay's self‑provisioned cert on first use |
| `node_id` | leave empty — the device auto‑mints and persists a random id |

(Equivalent `[relay]` keys in `agent.toml` if you edit config directly.)

Leave `node_id` empty: an auto‑minted random id is unguessable, which is what
keeps strangers from probing your device through the relay. The device then
keeps a persistent outbound connection to the relay and is reachable by its node
id from anywhere.

### Security notes

- `tofu = true` trusts the first relay certificate it sees. For a stronger
  pin, set `relay_ca_path` to the relay's CA instead. Never use
  `relay_insecure = true` outside local testing.
- The admission `token` only gates *registration* with the relay; the agent's
  own **pairing token** still gates every dashboard/agent route, relay or not.
- The relay sees only ciphertext and your node id's traffic metadata, never the
  request contents.
