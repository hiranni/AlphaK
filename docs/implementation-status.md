# AlphaK MVP · Implementation Status

| Field | Value |
| --- | --- |
| Date | 2026-10-22 |
| Change | `alphak-platform-mvp` |
| Progress | 100 / 111 tasks |
| Demo data | Seed retained for bootstrap; market APIs exclude demo rows and use live public-source data |

## Implemented

- Rust/Axum API with SQLite (rusqlite), unified `{ code, message, data, request_id }` envelope.
- Endpoints: health, market overview, search, stock detail, bars + MA/EMA/BOLL/MACD/KDJ/RSI, fundamentals, company profile, daily basic, index/sector detail, watchlists, settings, data jobs, quality.
- Deterministic demo seed: instruments, calendar, 260 daily bars, daily basic, financial reports/indicators, company profiles, indices/members, sectors/members, watchlist, quality/jobs.
- React/Vite pages: market dashboard, stock research, index/sector detail, watchlist, data admin, settings, 404.
- Hallmark “Paper Ledger” design system: `tokens.css`, editorial typography, responsive 320–1440, no horizontal page overflow.
- Watchlist: groups, add/remove, note, CSV import/export, polling; settings: theme/up-color/default home/timezone/density/poll; up-color preference applies globally.
- Backup/restore scripts with SHA-256 and SQLite integrity check.
- Financial report immutable versions + point-in-time query (seeded correction for 600519).
- Sync job state machine/idempotency, quality recomputation, market snapshot cache.
- Live cache: 60s snapshots, 60s minute lines/metrics, 15m history refresh, scoped retry queue and source cooldown.
- Market page filters demo rows; sector heat, popularity, and news are served from Eastmoney/Sina fallbacks with stale timestamps.
- Adapter trait + offline mock fixture, symbol normalization, qfq/hfq adjustment.
- Packaged runtime: `make build` → `dist/alphak/`, `scripts/start.sh` / `stop.sh`, pre-migration backup.
- AxData service at `/Users/ryan/Desktop/Project/AxData` with `.venv`; start/stop via `axdata.sh start|stop|status|restart|logs`.
- High-frequency paths: watchlist batch snapshots every 3s; stock-detail `5s/15s/30s/60s` AxData seconds K every 5s; no WebSocket.
- Financial/profile/valuation supplement endpoints with independent retry and cache TTLs.
- OpenAPI 3.1 endpoint + generated TypeScript client, watchlist rename/drag reorder/refresh/field preferences.
- Virtual scrolling for 500-row watchlists; volume moving averages and golden indicator tests.
- Bars qfq/hfq now applied before aggregation/indicators; as_of bounds reports, indicators, valuation; board-specific limit rules; trailing 52-week window; restore validates checksum/schema and rolls back.
- Tests: 84 Rust tests; frontend `tsc + vite build` passes; 10 Playwright E2E tests (including request-race, CSV round-trip, settings, chart sync, retry, OpenAPI contract, 500-row windowing).

## Intentional deviations (reported)

- Backend is **Rust + Axum** per user request, not FastAPI.
- Frontend is **Vite + React + hand-written CSS**, not Next.js/Tailwind/shadcn (user allowed random frontend).
- SQLite is the only database; no PostgreSQL/TimescaleDB/Redis.
- Direct host run; no Docker. Single-process scheduler; no Redis/Worker.
- AxData HTTP provider is primary for daily/index when `AXDATA_API_BASE` is set; Tencent/Eastmoney/Sina remain direct fallbacks. Demo seed remains only as bootstrap data.
- OpenAPI client is generated for market overview; other endpoints remain hand-typed until the spec gains full response schemas.

## Not implemented (remaining 11 tasks)

- Real financial/reference adapters (3.2–3.4, 4.15), source ToS review (10.8).
- CI (1.6), metrics/alerts (5.10), single-source failure isolation (6.6), security scan (10.7), browser matrix (11.4), final CI acceptance (11.5).
- 1,000-row CSV import verification.
- V1 strategy/virtual-account/Agent/replay capabilities, remote auth, and full backup UI.

## Verify

```bash
cargo test --manifest-path apps/api/Cargo.toml
cd apps/web && npm run build
```
