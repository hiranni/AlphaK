# Security Self-Check

Date: 2026-10-21. Scope: local demo slice. This is **not** a production security audit.

| Check | Result | Evidence |
| --- | --- | --- |
| Default bind is loopback | Pass | `ALPHAK_BIND` defaults to `127.0.0.1:8080` |
| Non-loopback refused | Pass | `ALPHAK_BIND=0.0.0.0:8099` exits 2 with a refusal message |
| No credentials in repo | Pass | `.env.example` contains placeholders only; `.env` is gitignored |
| Log redaction | Pass | `security::redact` covers token/api_key/authorization/cookie; request span uses redacted URI |
| Frontend production dependencies | Pass | `npm audit --omit=dev` reports 0 vulnerabilities |
| Rust dependency advisory scan | Pending | `cargo-audit` was not available in the environment at check time |
| Auth / TLS | Not in MVP | Local single-user loopback only; remote access is V1 |
| Rate limiting | Not in MVP | No public endpoint; real data source limits must be added with the adapter |

## Residual risks

- SQLite is a local file; protect the host account and backups.
- Demo data is synthetic; no real-source ToS review has been completed.
- The API has no authentication by design; do not expose it beyond loopback.
- Run `cargo audit` and `npm audit` in CI before enabling a real data source.
