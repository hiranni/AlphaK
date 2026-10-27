# Data Sources · Multi-Source Policy and ToS Guardrails

## Scope

AlphaK is personal, non-commercial research software. It uses open/free public
interfaces from Tencent, Eastmoney, Sina, CNINFO, and exchange pages. These are
not official licensed feeds: upstream terms, rate limits, and redistribution
rules still apply. Do not resell or redistribute raw data.

AxData is used only as an **endpoint/field reference**. AlphaK does not install
or run AxData, does not copy its research-only source code, and does not add its
Python/DuckDB/Parquet runtime.

## Dataset-level priority (current runtime)

Priority is configured per dataset. There is no single shared fallback chain.

| Dataset | Primary | Secondary | Fallbacks | Current scope |
| --- | --- | --- | --- | --- |
| `quote_daily` | AxData HTTP | Tencent | Eastmoney, Sina (raw only) | imported stocks + watchlist |
| `snapshot` | Tencent | AxData HTTP | Eastmoney, Sina | imported stocks + 5 indices |
| `index_quote` | AxData HTTP | Tencent | Eastmoney, Sina | SSE Composite, CSI 300, CSI 500, SZ Component, ChiNext |

AxData is enabled when `AXDATA_API_BASE` is set. AlphaK calls
`POST /v1/request/{interface}` and maps `stock_kline_daily_tdx`,
`index_kline_tdx`, and `stock_realtime_snapshot_tdx` into its own DTOs. If
AxData is down, the configured direct-source fallbacks take over. AxData keeps
its own `data/` Parquet/DuckDB workspace; AlphaK still writes only SQLite.

The market page reads only non-demo rows. The background loop refreshes snapshots
every 60 seconds, history every 15 minutes, minute bars/metrics every 60 seconds,
and sector/popularity/news caches every 60 seconds. Missing stock history is
fetched on first detail load (last year, bounded retry).

CNINFO is for financial/announcement data in a later batch. TDX is deferred.

The priority was set after the 2026-10-21/22 live sample: Tencent returned
consistently, while Eastmoney daily/snapshot began returning
`Empty reply from server`. Eastmoney remains implemented as the secondary source
and can be promoted again after a clean probe.

## AxData HTTP API

AxData runs separately at `/Users/ryan/Desktop/Project/AxData` with a project
`.venv`:

```bash
cd /Users/ryan/Desktop/Project/AxData
./axdata.sh start
./axdata.sh status
curl http://127.0.0.1:8666/health
```

AlphaK configuration:

```bash
AXDATA_API_BASE=http://127.0.0.1:8666
# AXDATA_API_TOKEN=  # only for remote AxData
```

AxData source requests are stateless and do not populate its core tables. Use
AxData collectors or `POST /v1/query` when you need cached/collected datasets.

## Live sample (2026-10-21/22)

`GET /api/v1/admin/sources/probe?symbol=600519.SH&start=2026-10-15&end=2026-10-21`

- Eastmoney daily: 5 rows, ~82 ms, close `1309.69`, volume `33514` (lots).
- Tencent daily: 5 rows, ~255 ms, close `1309.69`, volume `33514` (lots).
- Tencent snapshot: date `2026-10-21`, close `1309.69`, amount `4,403,820,000`.
- Eastmoney snapshot: intermittent empty reply; the client falls back to the last
  completed daily bar within the same source, and then to Tencent/Sina if needed.
- Sina daily/index/quote were reachable; Sina volume is shares (multiply by 100
  when comparing with Eastmoney/Tencent lots).
- CNINFO home was reachable; its announcement API needs a correct POST body and
  is out of the current cache batch.

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

### 大盘涨跌分布（2026-10-22）

- `GET /api/v1/market/breadth`：返回标准 envelope；其中 `data` 包含 `{ data: MarketBreadth | null, stale, error }`。
- 上游为乐咕乐股 `/stockdata/market-activity` 聚合页面（与 AKShare `stock_market_activity_legu()` 同源），不拉取全市场个股，也不执行全市场扫描。
- `MarketBreadth` 包含 `advance/decline/flat/suspended/limit_up/limit_down/as_of/source/scope`。页面主标题为“大盘”，范围说明在提示中保留；不声称已验证北交所覆盖。
- 缓存存于 SQLite `live_cache`，成功结果缓存 120 秒；失败后 30 秒再试，保留上次成功数据，重启后可恢复。备用 hostname 只解决同源连接问题，不代表独立上游容灾。
- 前端每 30 秒查询缓存；上涨占比的分母为上涨 + 下跌 + 平盘，排除停牌。市场日期与本地样本日期不一致时不显示占比差异，两个统计时间分别标注。

### 腾讯历史日线入口降级（2026-10-22）

历史日线优先使用 `https://proxy.finance.qq.com/ifzqgtimg/appstock/app/fqkline/get`，连接或内容解析失败后尝试原 `https://web.ifzq.gtimg.cn/appstock/app/fqkline/get`。两个入口使用相同的腾讯日线字段及成交量口径，均支持原始日线和前复权日线；本次原入口实测返回 WAF HTML，替代入口成功返回一年范围的日线。仅在取得匹配日期的真实复权价格后计算因子，不以默认因子伪造复权数据。

### High-frequency paths

- Watchlist: the visible rows are polled via `GET /api/v1/realtime/quotes` every 3 seconds. AlphaK sends one batch `stock_realtime_snapshot_tdx` request (max 20 symbols), not one request per symbol.
- Stock detail: periods `5s/15s/30s/60s` use AxData `stock_kline_second_tdx`. AlphaK keeps 360 bars in `live_cache`, polls every 5 seconds, and does not use WebSocket.
- Side effect: a seconds request currently fetches AxData's full seconds history (about 5 MB for one symbol) and trims to 360 bars locally. This is acceptable for one open chart but must not be looped over every watchlist symbol.

### 行情刷新修订（2026-10-25）

- 个股页面仅展示实时（3s）、5分、30分、日K、月K；默认实时。3秒柱由 AxData 1秒柱按时间桶聚合 OHLC、成交量和成交额，HTTP 每3秒轮询，缓存3秒，不承诺上游延迟。
- 自选全部标的每3秒启动一轮刷新，每批最多20只，批次串行；上一轮未结束不启动重叠轮询。后台标签暂停。一批失败不阻止其他批次；失败标的保留上次值。耗时超过3秒时实际周期延长。
- 实时批量报价成功后写入 `quote_snapshot`，每个标的覆盖最新值，不保存每3秒的完整历史。秒/分钟K线存于 SQLite `live_cache`，秒线仅保留最近360根；日线历史存于 `quote_daily`。
- 市场概览忽略周末及日历明确标记休市日期的行情，避免上游错误日期使最近有效交易日样本消失。不自动删除已有异常日期记录。
- 规则符合分、趋势、信号和量比仍按日线分析，不能把3秒轮询描述为每3秒重新计算日线策略。

## 持久化优化与保留管理

唯一数据保留设计为 `openspec/changes/data-retention-and-sqlite-performance/design.md`。`make harness` 用内存迁移校验表结构与政策；修改表、字段、索引或政策后，必须先更新设计的用途/期限/删除规则，再执行 `make persistence-doc-update` 并审阅差异。`make build` 和CI强制检查。

实时报价最多20只一个事务，先验证整批；hash去重报价内容。上游没有可信观测时间时，provider_time/updated_at为null，last_checked_at仅表示本机检查时间。秒/分钟图使用bar_cache增量替换时间桶，360/1024根上限，成交量不反复累计；旧live_cache JSON保留供回滚。上游仍可能返回整段历史，本地增量不等于网络增量。

`GET /api/v1/data/retention` 返回实际路径、表行数、SQLite版本、PRAGMA、DB/WAL体积及政策，不输出凭据。清理使用 `POST /api/v1/data/cleanup/preview` → `POST /api/v1/data/cleanup`；后者必须传plan_id，dry_run默认true，显式false才执行。计划10分钟过期；仅白名单显示缓存和bar允许删除，未知dataset、任务、水位、用户配置和研究输入不删。自动清理尚未启用。

`POST /api/v1/data/inputs` 冻结JSON输入，`GET /api/v1/data/inputs/{hash}`校验读取，文件缺失或不匹配返回INPUT_UNAVAILABLE；输入目录由ALPHAK_INPUT_DIR指定，默认与ALPHAK_DB同名.inputs目录，必须与DB一起备份。备份脚本现已同时保存SQLite、被manifest引用的输入包、校验和与脱敏manifest；恢复前必须停服务，先创建当前状态恢复点，并进入维护状态，自动清理保持关闭。详细解释包8季度清理与孤儿文件回收尚未启用。

### 涨跌分布曲线的时间口径（2026-10-25）

`GET /api/v1/market/breadth/history?period=30min|1h|daily|weekly|monthly`：

- 日线样本按实际连续交易日可比收盘价计算，排除演示、休市及零成交量记录；响应 `members` 为最新可比日的成分入口，与工作台用户分组关注的研究集合分别标注。
- 分时样本只读取已有 `bar_cache` 中的分钟/秒级行情，按北京时间桶取每股最后价与此前真实日线收盘价比较；响应每点家数就是实际覆盖数，不承诺覆盖完整日线股票池，不触发全市场扫描。
- 大盘按 `market_breadth_observation.provider_time` 分桶，不用抓取时间冒充行情时间；缺少发布时间时不进入分时序列。
- 周/月取周期内最后一次真实观测，不求和上涨家数；保留 `observed_date`。曲线平滑仅为视觉连接，圆点表示真实观测，缺失不插值、不回填。
- 现有大盘数据源提供当前聚合快照；此接口没有取得新的历史数据。只有一个观测点时不能产生连续曲线。前端展示观测点数量和覆盖数。
- 分时覆盖不足最新日线成分的 80% 时，前端标记“局部缓存”，不计算大盘差异；80% 是展示完整性门槛，不是统计有效性的保证。
- 本次只读既有持久化表，不增加表结构或更改留存策略。

### 大盘 AxData 优先与样本统一（2026-10-25）

- 当前聚合快照优先 AxData `cls_market_emotion`（财联社），失败回退乐咕乐股；财联社未给行情时间，因此 `as_of` 留空、仅保留最新缓存，不伪造日期、不写研究历史。
- 历史按需后台请求 AxData `index_kline_tdx`，上证综合 `000001.SH` 和深证 A 指 `399107.SZ`，严格按同一 `trade_time` 合并涨跌家数；30m、60m、day 分别采集，周/月从日线取最后有效观测。每周期两请求、120秒合并窗口，不扫描成分股票。首次加载可等待下一次30秒页面刷新。
- 通达信不提供平盘家数，历史保存 `flat_known=false`。对比曲线统一使用上涨/(上涨+下跌)，而首页含平盘的分布卡仍显示原始三项统计；不将未知平盘解释为真实0。
- 全部样本初始化从最新有效日线可比成分一次性补入 `watchlist-all`，保留原37只及分组/备注；以 `app_setting.sample.collection.v1` 记录完成状态，用户后续删除不会被每次打开页面自动恢复。
