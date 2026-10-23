# Deployment & Operations

## Current slice (verified)

```bash
cargo test --manifest-path apps/api/Cargo.toml
make build
scripts/start.sh  # backs up an existing DB before startup migrations
scripts/stop.sh
```

- API: `127.0.0.1:8080` (non-loopback is refused).
- Web: `127.0.0.1:8080`, built static files served by the same process as the API.
- Package: `dist/alphak/`; can be copied to the same OS/CPU architecture and run with its `scripts/start.sh` / `scripts/stop.sh`.
- Logs/PID: `dist/alphak/logs/alphak.log` and `dist/alphak/run/alphak.pid` when launched from the checkout.
- Database: SQLite, default `./alphak.db`; override with `ALPHAK_DB`.
- Demo seed: bootstrap only. Market APIs exclude `source='demo'`; live public-source rows and caches supply the dashboard and stock pages.
- Background refresh: snapshots/metrics/minute caches every 60s, history every 15 minutes, sector/popularity/news caches every 60s.
- Packaged frontend is served by the API from `dist/alphak/web`; no Node.js is required at runtime.

## Environment

```bash
ALPHAK_BIND=127.0.0.1:8080
ALPHAK_DB=./alphak.db
RUST_LOG=alphak_api=info
```

## Backup / restore

```bash
scripts/backup.sh
scripts/restore.sh backups/alphak-<timestamp>.db
```

Restore verifies SHA-256, backs up the current database, runs `PRAGMA integrity_check`, then restores. Corrupt backups are refused.

## Runtime boundary

- Direct host run only: `cargo run` for the API and `npm run dev` for the web app.
- No Docker, Compose, PostgreSQL, TimescaleDB, or Redis.
- SQLite is the only database; keep `ALPHAK_DB` on local disk and back it up.
- Only loopback binding is supported.

## Data source configuration

Current runtime uses open/free public endpoints: Tencent, Eastmoney, and Sina for
quotes; Eastmoney for financial/profile/metrics supplements; Sina for news and
sector fallback. Priority is per dataset and bounded retries/cooldowns apply.
No AxData/Python runtime and no extra database are used. Public endpoints are
not official licensed feeds: personal research only, no resale/redistribution,
and upstream ToS/rate limits still apply.

## Quality and freshness

- `GET /api/v1/admin/quality` recomputes coverage/missing/duplicates/anomalies.
- Snapshot and minute/metric caches target 60 seconds; history refresh targets 15 minutes.
- Failed scopes are persisted with their symbol/date range and retried by the background worker.
- If a source is late or all sources fail, the last successful data is retained with a stale timestamp; demo rows are never substituted.

## OpenAPI

`GET /api/v1/openapi.json` serves the contract in `docs/api/openapi.json`.
