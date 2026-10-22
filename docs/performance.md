# Performance Snapshot

Date: 2026-10-21. Environment: local Mac, Chrome headless, Vite dev + Rust API, demo SQLite. Dev-mode numbers are not production SLOs; they are acceptance evidence for this slice.

## Dashboard LCP

- Measured with PerformanceObserver (`largest-contentful-paint`) at 1280×900.
- **LCP: 1,044 ms** (target ≤ 2,500 ms). Wall time to network idle: 2,470 ms.

## API latency (25 requests each)

| Endpoint | p50 | p95 | max |
| --- | ---: | ---: | ---: |
| `/api/v1/health` | 0.5 ms | 0.7 ms | 7.5 ms |
| `/api/v1/market/overview` | 0.6 ms | 0.8 ms | 0.9 ms |
| `/api/v1/instruments/600519.SH` | 0.7 ms | 0.7 ms | 0.9 ms |
| `/api/v1/instruments/600519.SH/bars` | 6.5 ms | 6.8 ms | 6.9 ms |

Targets: detail first-screen API P95 ≤ 500 ms; 5-year daily bar preparation ≤ 1.5 s. Both pass on the demo dataset.

## Notes

- Overview is served from the local `market_snapshot` cache; `refresh=true` recomputes it.
- First request after a cold start includes SQLite page cache warm-up.
- Run this again before production with a real dataset and production build (`npm run build` + static server).
