# Credential broker verification (L0, dev host)

Date: 2026-10-08. Binary: `modules/credential-broker/broker` release build (24.9 s).
Device image not rebuilt for this stage; the unit is verified with `systemd-analyze` only
(`scripts/verify-units.sh`, 4 units) and ships with the next image build.

## Automated: `cargo test` -- 16 tests, 0 failures

| Area | Test | Checks |
|---|---|---|
| store | `roundtrip_and_tamper` | encrypt/decrypt; a flipped ciphertext byte is rejected; a second open reads the same key |
| store | `key_file_mode_is_private` | key created 0600; a 0644 key file is refused at start |
| store | `store_persists_credentials_and_grants` | survive reopen; `store.json` never contains the plaintext |
| store | `approvals_are_single_use` | consumed on use; wrong hash does not match |
| policy | `paths`, `methods_default_get_only`, `hosts`, `hash_...` | prefix match, `..` rejected, GET-only default, `*.suffix` is strict subdomain, hash is query-order independent and body sensitive |
| config | `rejects_plain_http_scope`, `approval_classes`, `shipped_config_parses_and_validates` | `deploy/config/broker.toml` loads: 12 scopes, 5 credential names |
| api | `scoped_call_injects_credential_and_enforces_grant_path_method` | mock vendor sees `Authorization: Bearer <token>` and the query; no-grant / wrong path / wrong method / other task -> 403 with reason; audit has `auto` and `refused:*` lines and never the token |
| api | `external_send_requires_single_use_approval_bound_to_request` | `external-send` -> 403 `approval-required` + 64-hex hash; approving another hash does nothing; approving the hash lets exactly one call through; audit line `approved` |
| api | `header_auth_and_missing_credential` | `header:X-Api-Key` injection; missing credential -> 409 before any upstream call |
| api | `non_admin_peer_cannot_manage_but_can_call` | a peer uid that is not the admin gets 403 on every admin route, can still read `/v1/health` and `/v1/call` |
| proxy | `egress_proxy_allows_listed_hosts_only_and_audits` | reqwest through the proxy reaches an allow-listed origin (200, query intact); a host not on the list gets 403 and is never contacted; CONNECT to it refused; audit lines for both |

Reverse check: removing the `..` guard in `policy::path_allowed` makes `policy::tests::paths` fail.

## L0 integration: the agent's traffic goes through the broker

```
scripts/l0-broker.sh build && scripts/l0-broker.sh start   # 12 scopes, egress on 127.0.0.1:3128
scripts/l0-agent.sh init && scripts/l0-agent.sh start      # agent.toml now carries [proxy]
curl -X POST -H "X-Pairing-Code: ..." :42617/pair          # pair, then
curl -X POST -H "Authorization: Bearer zc_..." -d '{"message":"Reply with one word."}' :42617/webhook
```

Result: the agent answered `{"error":"LLM request failed"}` (no Console key on this host,
expected) and the broker's audit log gained

```
{"ts":"2026-10-08T15:47:17Z","task":"","scope":"egress","method":"CONNECT","host":"console.redrob.ai",
 "path":"","status":200,"decision":"auto",...}
```

i.e. the inference call left the agent only through the broker's proxy. `zeroclaw doctor`
accepts the `[proxy]` block (its remaining errors are the missing local router and the
Console listing, same as before this stage). Idle, the agent makes no outbound call.

## Not verified

- On the device: unit start as `redrob-broker`, socket group permissions, `RuntimeDirectory`,
  the agent reaching `/run/redrob/broker.sock` -- needs the next image build (Stage 5).
- A real vendor call with a real token (Gmail, Slack): the mock vendor only proves header
  injection and path/method policy.
- Sandbox (Z2) egress: no Podman/gVisor on this host.
