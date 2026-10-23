import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowUpRight, Database, RotateCw } from "lucide-react";
import { api } from "../api";
import type { MarketOverview, NewsFeed, QualityReport, SourceStatus } from "../types";
import { Change } from "../components/Change";
import { ErrorState, Loading } from "../components/States";
import { dateLabel, money, num, pct } from "../format";

function newsTime(epoch: number) {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000) - epoch);
  if (seconds < 60) return "刚刚";
  if (seconds < 3600) return `${Math.floor(seconds / 60)} 分钟前`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} 小时前`;
  return new Date(epoch * 1000).toLocaleDateString("zh-CN", { timeZone: "Asia/Shanghai", month: "2-digit", day: "2-digit" });
}

export function Dashboard() {
  const [marketBreadth, setMarketBreadth] = useState<Awaited<ReturnType<typeof api.marketBreadth>> | null>(null);
  const [breadthError, setBreadthError] = useState("");
  const [overview, setOverview] = useState<MarketOverview | null>(null);
  const [popular, setPopular] = useState<Awaited<ReturnType<typeof api.popularity>> | null>(null);
  const [news, setNews] = useState<NewsFeed | null>(null);
  const [heatMode, setHeatMode] = useState<"high" | "low">("high");
  const [quality, setQuality] = useState<QualityReport[]>([]);
  const [sources, setSources] = useState<SourceStatus | null>(null);
  const [error, setError] = useState("");
  const [syncing, setSyncing] = useState(false);
  const [syncNotice, setSyncNotice] = useState("");
  const navigate = useNavigate();

  const syncMarket = async () => {
    setSyncing(true);
    setSyncNotice("");
    try {
      setOverview(await api.refreshOverview());
      setSyncNotice("已请求同步，结果自动更新");
    } catch (err) {
      setSyncNotice(err instanceof Error ? err.message : "同步失败");
    } finally {
      setSyncing(false);
    }
  };

  const load = () => {
    setError("");
    Promise.all([
      api.overview().then(setOverview),
      api.popularity().then(setPopular).catch(() => null),
      api.news().then(setNews).catch(() => null),
      api.quality(),
      api.sourceStatus().catch(() => null),
    ])
      .then(([, , , nextQuality, nextSources]) => {
        setQuality(nextQuality);
        setSources(nextSources);
      })
      .catch((err: Error) => setError(err.message));
  };

  useEffect(() => {
    const loadBreadth = () => api.marketBreadth().then((value) => { setMarketBreadth(value); setBreadthError(""); }).catch(() => setBreadthError("大盘统计更新失败，稍后自动重试"));
    loadBreadth();
    const breadthTimer = window.setInterval(loadBreadth, 30000);
    load();
    const timer = window.setInterval(load, 10000);
    return () => { window.clearInterval(timer); window.clearInterval(breadthTimer); };
  }, []);

  if (error && !overview) return <ErrorState message={error} onRetry={load} />;
  if (!overview) return <Loading label="正在加载行情" />;

  const { breadth, indices, sectors } = overview;
  const total = breadth.advance + breadth.decline + breadth.flat;
  const advancePct = total ? (breadth.advance / total) * 100 : 0;
  const market = marketBreadth?.data;
  const marketTotal = market ? market.advance + market.decline + market.flat : 0;
  const marketPct = market && marketTotal ? market.advance / marketTotal * 100 : 0;
  const sameDate = !!market && market.as_of.slice(0, 10) === overview.as_of;
  const sortedSectors = [...sectors].sort((left, right) => right.change_pct - left.change_pct);
  const visibleSectors = heatMode === "high" ? sortedSectors.slice(0, 8) : [...sortedSectors].reverse().slice(0, 8);
  const bestSector = sortedSectors[0];
  const worstSector = sortedSectors[sortedSectors.length - 1];
  const dataHealth = quality.find((item) => item.dataset === "quote_daily");

  return (
    <div className="page dashboard market-screen">
      <header className="page-head">
        <div className="page-head-copy">
          <h1 className="display">市场行情</h1>
          <span className="market-meta mono">{overview.as_of || "等待首次同步"} · 每分钟更新 · {overview.source === "pending" ? "暂无真实行情" : overview.source}</span>
        </div>
        <aside className="head-aside">
          {syncNotice && <span className="muted" role="status">{syncNotice}</span>}
          <button className="btn btn-soft" disabled={syncing} onClick={syncMarket}><RotateCw size={14} /> {syncing ? "提交中…" : "同步行情"}</button>
        </aside>
      </header>

      {error ? <div className="data-warning" role="status">更新失败，保留上次行情：{error}</div> : null}

      {!indices.length && <div className="data-warning" role="status">正在同步真实指数行情；上游不可用时保留最后成功数据。可在数据页查看同步结果。</div>}

      {sources?.freshness.some((item) => item.status !== "fresh") ? (
        <div className="data-warning" role="status">
          <Database size={14} aria-hidden="true" />
          <span>
            部分行情待更新：{sources.freshness.filter((item) => item.status !== "fresh").map((item) => `${({ index_quote: "指数", quote_daily: "个股", snapshot: "快照" } as Record<string, string>)[item.dataset] ?? item.dataset} ${item.last_success_at ? `截至 ${dateLabel(item.last_success_at)}` : "未同步"}`).join(" · ")}
          </span>
        </div>
      ) : null}

      <section className="ticker-row" aria-label="指数看板">
        {indices.map((index) => (
          <button key={index.code} className="ticker" onClick={() => navigate(`/index/${index.code}`)}>
            <span className="ticker-name">{index.name}</span>
            <span className="ticker-price mono">{num(index.close, 2)}</span>
            <span><Change value={index.change_points} points /> · <Change value={index.change_pct} /></span>
            <span className="ticker-amount mono">成交 {index.amount_known ? money(index.amount) : "—"}</span>
            <ArrowUpRight size={14} className="ticker-arrow" aria-hidden="true" />
          </button>
        ))}
      </section>

      <div className="dash-grid">
        <section className="panel breadth-panel breadth-comparison">
          <div className="panel-head">
            <h2>大盘与样本涨跌分布</h2>
            <span className="panel-note">红涨 · 绿跌 · 灰平</span>
          </div>
          {[{ label: "大盘", data: market, count: marketTotal, time: market?.as_of, note: market ? `${market.source} · ${marketBreadth?.stale || breadthError ? "更新失败，保留上次统计" : "最新统计"}` : breadthError || marketBreadth?.error || "正在获取大盘统计…" },
            { label: "样本", data: total ? breadth : null, count: total, time: overview.as_of, note: "本地真实行情" }].map((row) => (
            <div className="breadth-compare-row" key={row.label} title={row.label === "大盘" ? market?.scope : "本地已有行情的股票样本，非全市场"}>
              <div className="breadth-row-heading"><strong>{row.label}</strong><span className="muted mono">{row.count ? `${row.count.toLocaleString()} 只` : "等待数据"}</span><span className="breadth-row-time muted" title={row.time}>{row.label === "大盘" ? row.time?.slice(11, 16) || "—" : row.time || "—"}</span></div>
              <div className="breadth-row-values mono">
                <span className="is-up">↑ 上涨 <b>{row.data?.advance.toLocaleString() ?? "—"}</b></span>
                <span className="is-down">↓ 下跌 <b>{row.data?.decline.toLocaleString() ?? "—"}</b></span>
                <span className="muted">平盘 <b>{row.data?.flat.toLocaleString() ?? "—"}</b></span>
                <span>上涨占比 <b>{row.data && row.count ? `${(row.data.advance / row.count * 100).toFixed(1)}%` : "—"}</b></span>
              </div>
              <div className="breadth-bar" role="img" aria-label={`${row.label}：${row.data ? `上涨 ${row.data.advance}，下跌 ${row.data.decline}，平盘 ${row.data.flat}` : "暂无数据"}`}>
                {row.data && row.count ? <><span className="breadth-up" style={{ width: `${row.data.advance / row.count * 100}%` }} /><span className="breadth-flat" style={{ width: `${row.data.flat / row.count * 100}%` }} /><span className="breadth-down" style={{ width: `${row.data.decline / row.count * 100}%` }} /></> : null}
              </div>
              <div className="breadth-row-note muted" title={row.note}>{row.note}{row.label === "大盘" && market ? ` · 涨停 ${market.limit_up} / 跌停 ${market.limit_down} · 停牌 ${market.suspended}` : ""}</div>
            </div>
          ))}
          <div className="breadth-comparison-foot">
            <span>{market && total && sameDate ? <>样本上涨占比{advancePct >= marketPct ? "高于" : "低于"}大盘 <b className="mono">{Math.abs(advancePct - marketPct).toFixed(1)}</b> 个百分点</> : market && total ? "统计日期不同，暂不比较上涨占比" : "大盘与样本数据齐备后显示占比差异"}</span>
            <span className="muted">样本成交额 {total && breadth.amount_known ? money(breadth.total_amount) : "—"}</span>
          </div>
        </section>

        <section className="panel sectors-panel">
          <div className="panel-head">
            <div>
              <span className="eyebrow mono">SECTORS</span>
              <h2>板块温度</h2>
            </div>
            <span className="panel-note">{overview.sector_source ? `${overview.sector_source === "sina" ? "新浪财经" : "东方财富"} · ${overview.sector_updated_at ? new Date(overview.sector_updated_at * 1000).toLocaleTimeString("zh-CN", { timeZone: "Asia/Shanghai", hour12: false }) : ""}${overview.sector_stale ? " · 更新失败，保留上次数据" : " · 每分钟更新"}` : "行业汇总 · 同步中"}</span>
          </div>
          {!sectors.length && <p className="empty-data">正在获取真实行业板块汇总，上游失败时保留最后成功数据。</p>}
          <div className="heat-toolbar"><div className="segmented" role="group" aria-label="板块热度模式">{[["high", "热度最高", "🔥"], ["low", "热度最低", "🧊"]].map(([value, label, icon]) => <button key={value} aria-pressed={heatMode === value} className={heatMode === value ? "is-active" : ""} onClick={() => setHeatMode(value as "high" | "low")}><span aria-hidden="true">{icon}</span> {label}</button>)}</div><span className="panel-note">{heatMode === "high" ? "按涨跌幅 · 前 8" : "按涨跌幅 · 后 8"}</span></div>
          <div className="table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th className="num">#</th>
                  <th>板块</th>
                  <th className="num">涨跌幅</th>
                  <th>领涨</th>
                  <th className="num">成交额</th>
                </tr>
              </thead>
              <tbody>
                {visibleSectors.map((sector, index) => (
                  <tr key={sector.id} className={sector.id > 0 ? "row-link" : undefined} onClick={sector.id > 0 ? () => navigate(`/sector/${sector.id}`) : undefined}>
                    <td className="num mono muted">{String(index + 1).padStart(2, "0")}</td>
                    <td>
                      <span className="cell-title"><span className={sector.change_pct > 0 ? "is-up" : sector.change_pct < 0 ? "is-down" : "muted"} role="img" aria-label={sector.change_pct > 0 ? "上涨" : sector.change_pct < 0 ? "下跌" : "平盘"}>{sector.change_pct > 0 ? "↑" : sector.change_pct < 0 ? "↓" : "→"}</span> {sector.name}</span>
                      <span className="cell-sub">{sector.sector_type === "industry" ? "行业" : "概念"}</span>
                    </td>
                    <td className="num"><Change value={sector.change_pct} /></td>
                    <td>
                      <span className="cell-title">{sector.leading_name || "—"}</span>
                      <span className="cell-sub mono">{sector.leading_symbol}</span>
                    </td>
                    <td className="num mono">{money(sector.turnover)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="sector-foot">
            <span>最强 <b>{bestSector?.name ?? "—"}</b> <span className="mono">{bestSector ? pct(bestSector.change_pct) : "—"}</span></span>
            <span>最弱 <b>{worstSector?.name ?? "—"}</b> <span className="mono">{worstSector ? pct(worstSector.change_pct) : "—"}</span></span>
          </div>
        </section>

        <div className="dash-rail">
          <aside className="panel watch-panel">
            <div className="panel-head">
              <div>
                <span className="eyebrow mono">POPULARITY</span>
                <h2>人气股</h2>
              </div>
              <span className="panel-note">TOP 12{popular ? ` · ${new Date(popular.updated_at * 1000).toLocaleTimeString("zh-CN", { timeZone: "Asia/Shanghai", hour12: false })}` : ""}</span>
            </div>
            <p className="panel-footnote">东方财富人气榜 · 按人气排名{popular?.stale ? " · 保留上次排行" : ""}</p>
            <div className="watch-list">
              {popular?.data.length ? popular.data.map((item) => (
                <button key={item.symbol} className="watch-row" onClick={() => navigate(`/stock/${item.symbol}`)}>
                  <span className={`popular-rank mono ${item.rank <= 3 ? "rank-leading" : ""}`}>{String(item.rank).padStart(2, "0")}</span>
                  <div>
                    <span className="cell-title">{item.name}</span>
                    <span className="cell-sub mono">{item.symbol}</span>
                  </div>
                  <div className="watch-numbers">
                    <span className="mono">{num(item.close)}</span>
                    <Change value={item.change_pct} />
                  </div>
                </button>
              )) : <p className="empty-data">正在获取人气排行，上游不可用时自动重试</p>}
            </div>
          </aside>

          <section className="panel news-panel" aria-label="热点新闻">
            <div className="panel-head">
              <div>
                <span className="eyebrow mono">NEWS</span>
                <h2>热点新闻</h2>
              </div>
              <span className="panel-note">{news ? `新浪财经 · ${news.stale ? "缓存" : "7×24"}` : "同步中"}</span>
            </div>
            <div className="news-list">
              {news?.data.length ? news.data.map((item) => (
                <a key={item.id} className="news-row" href={item.url} target="_blank" rel="noreferrer">
                  <span className="news-time mono">{newsTime(item.published_at)}</span>
                  <span className="news-title">{item.title}</span>
                </a>
              )) : <p className="empty-data">正在获取热点新闻，上游不可用时自动重试</p>}
            </div>
          </section>
        </div>

        <aside className="panel health-panel">
          <div className="panel-head">
            <div>
              <span className="eyebrow mono">DATA HEALTH</span>
              <h2>数据体检</h2>
            </div>
            <Database size={17} aria-hidden="true" />
          </div>
          <div className="health-score">
            <strong className="mono">{dataHealth ? `${(dataHealth.coverage * 100).toFixed(1)}%` : "—"}</strong>
            <span>日线覆盖率</span>
          </div>
          <ul className="health-list">
            {quality.slice(0, 4).map((item) => (
              <li key={item.dataset}>
                <span>{item.dataset}</span>
                <span className="mono">{(item.coverage * 100).toFixed(1)}%</span>
              </li>
            ))}
          </ul>
          <button className="text-link" onClick={() => navigate("/data")}>打开数据管理 <ArrowUpRight size={13} /></button>
        </aside>
      </div>
    </div>
  );
}
