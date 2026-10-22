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
- Demo seed: 13 instruments, 260 trading days. All quotes are synthetic.

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

MVP production target: AkShare only. Tushare/BaoStock are interface placeholders; a real second source is V1. Never commit credentials; use environment/secret storage.

## Quality and freshness

- `GET /api/v1/admin/quality` recomputes coverage/missing/duplicates/anomalies.
- Daily sync target: 15:10 start, 17:00 deadline, 18:00 reconciliation.
- If source is late, status is “waiting for source” and the page keeps the last data timestamp.

## OpenAPI

`GET /api/v1/openapi.json` serves the contract in `docs/api/openapi.json`.
