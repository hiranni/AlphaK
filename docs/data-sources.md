# Data Sources · Multi-Source Policy and ToS Guardrails

## Scope

AlphaK is personal, non-commercial research software. It uses open/free public
interfaces from Tencent, Eastmoney, Sina, CNINFO, and exchange pages. These are
not official licensed feeds: upstream terms, rate limits, and redistribution
rules still apply. Do not resell or redistribute raw data.

AxData is used only as an **endpoint/field reference**. AlphaK does not install
or run AxData, does not copy its research-only source code, and does not add its
Python/DuckDB/Parquet runtime.

## Dataset-level priority (first batch)

Priority is configured per dataset. There is no single shared fallback chain.

| Dataset | Primary | Secondary | First-batch scope |
| --- | --- | --- | --- |
| `quote_daily` | Tencent | Eastmoney | 3 stocks × 5 trading days |
| `snapshot` | Tencent | Eastmoney | same stocks + 2 indices |
| `index_quote` | Tencent | Eastmoney | SSE Composite, CSI 300 |

Sina is a cross-check source, not part of the first-batch priority. CNINFO is for
financial/announcement data in a later batch. TDX is deferred.

The priority was set after the 2026-10-21/22 live sample: Tencent returned
consistently, while Eastmoney daily/snapshot began returning
`Empty reply from server`. Eastmoney remains implemented as the secondary source
and can be promoted again after a clean probe.

## Live sample (2026-10-21/22)

`GET /api/v1/admin/sources/probe?symbol=600519.SH&start=2026-10-15&end=2026-10-21`

- Eastmoney daily: 5 rows, ~82 ms, close `1309.69`, volume `33514` (lots).
- Tencent daily: 5 rows, ~255 ms, close `1309.69`, volume `33514` (lots).
- Tencent snapshot: date `2026-10-21`, close `1309.69`, amount `4,403,820,000`.
- Eastmoney snapshot: intermittent empty reply; the client falls back to the last
  completed daily bar within the same source, and then to Tencent if needed.
- Sina daily/index/quote were reachable; Sina volume is shares (multiply by 100
  when comparing with Eastmoney/Tencent lots).
- CNINFO home was reachable; its announcement API needs a correct POST body and
  is out of the first-batch scope.

The sample is not a stability guarantee. A source is only marked healthy after a
successful response with parseable rows and passing quality checks.

## Success contract

A source sync succeeds only when:

1. The requested date range and symbols are covered (gaps are retried once and
   reported as `gap_count`);
2. OHLC, volume, and amount pass validation;
3. The batch is written in one SQLite transaction with `source`, `batch_id`, and
   `fetched_at`;
4. The publication is scoped to the requested symbols and dates.

Partial success is allowed and recorded as `partial`. When every source fails:

- the last successful rows stay readable;
- `data_freshness.status` becomes `stale`;
- the UI shows the last success time and source;
- **no mock, demo seed, or synthetic fallback is generated**.

## Raw prices and adjustment factors

Raw OHLCV is stored in `quote_daily`. `adj_factor` is derived from a separate
qfq request and recorded with `adj_source` / `adj_batch_id`. If the factor fetch
fails, raw prices still publish and `adj_pending` is reported. The schema keeps
the sentinel `1.0` for compatibility, but `adj_source` stays empty; the bars API
degrades qfq/hfq to `none`, sets `adjust_available=false`, and returns a notice
instead of treating the sentinel as a valid factor.

Tencent daily responses do not carry amount. `amount_known=false` is stored;
an unknown amount never overwrites an existing known amount. New unknown rows
are exposed as unavailable rather than “zero turnover”. Index rows follow the
same rule.

## Scoped retry queue

A failed or incomplete request is persisted as
`(dataset, symbol, start_date, end_date)` in `source_retry_task`. The automatic
retry worker replays that exact scope, not a default watchlist or a hard-coded
ten-day window. A task is cleared only after its publication transaction
succeeds and no requested trading day remains missing; snapshot tasks use an
empty date range. Freshness is marked `fresh` only when no task remains for the
dataset.

## Failure and cooldown

- Each source gets two bounded attempts with a short backoff.
- Persisted retry tasks retain the original symbol/date scope and are replayed
  by the background worker; publish errors are not treated as success.
- A failing source enters exponential cooldown (15s, 30s, … capped at 120s).
- The dataset falls back only to its own configured secondary source.
- Manual retry is available; automatic retry is handled by the source scheduler.

## ToS checklist (before widening a source)

- [x] Personal research use only; no resale, no public redistribution.
- [x] Rate limits, bounded retries, and cooldowns documented.
- [x] Source label and fetch time stored with the data.
- [x] No credentials in logs, API responses, or git.
- [ ] Per-upstream ToS/robots/redistribution review recorded (10.8).
- [x] Source can be disabled without breaking cached read paths.
