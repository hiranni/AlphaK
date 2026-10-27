/* Hallmark · Workbench · existing AlphaK terminal tokens · pre-emit critique: P5 H5 E4 S5 R5 V3 */
import { useEffect, useState } from "react";
import { LayoutGrid, Minus, Activity, ArrowUpRight, Bell, ChevronRight, TrendingUp, TrendingDown, ChartNoAxesCombined, Radar, Database, ShieldCheck, Zap, Waves, TriangleAlert } from "lucide-react";
import { api } from "../api";
import type { MarketOverview, WatchAnalysis, WatchlistGroup, WatchWorkbench } from "../types";
import { num, pct, changeClass } from "../format";
import { Change } from "./Change";

function signalTone(kind: string) {
  return ["breakout", "reversal", "momentum"].includes(kind) ? "bullish" : kind === "support_break" ? "bearish" : kind === "volatility" ? "risk" : "neutral";
}

export function SignalTitle({ kind, title }: { kind: string; title: string }) {
  const tone = signalTone(kind);
  const label = { bullish: "偏多", bearish: "偏空", risk: "风险", neutral: "中性" }[tone];
  const Icon = tone === "bullish" ? TrendingUp : tone === "bearish" ? TrendingDown : tone === "risk" ? TriangleAlert : ChartNoAxesCombined;
  return <span className={`signal-title signal-tone-${tone}`}><span className="signal-type-icon"><Icon size={18} aria-hidden="true" /></span><span>{title}</span><small className="signal-tone-label">{label}</small></span>;
}

export function Sparkline({ values }: { values: number[] }) {
  if (values.length < 2) return <span className="muted">—</span>;
  const min = Math.min(...values); const span = Math.max(...values) - min || 1;
  const points = values.map((v, i) => `${i / (values.length - 1) * 100},${26 - (v - min) / span * 24}`).join(" ");
  return <svg className={`watch-spark ${changeClass(values.at(-1)! - values[0])}`} viewBox="0 0 100 28" role="img" aria-label="最近 30 根有效复权日线走势"><polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.5" vectorEffect="non-scaling-stroke" /></svg>;
}

export function WatchOverview({ groups, data, error, onRetry }: { groups: WatchlistGroup[]; data: WatchWorkbench | null; error: string; onRetry: () => void }) {
  const [market, setMarket] = useState<MarketOverview | null>(null);
  const [marketError, setMarketError] = useState("");
  const [indexCode, setIndexCode] = useState("000001.SH");
  const [clock, setClock] = useState(new Date());
  const load = () => { api.overview().then(v => { setMarket(v); setMarketError(""); }).catch(() => setMarketError("指数暂不可用")); };
  useEffect(() => { load(); const timer = window.setInterval(() => { setClock(new Date()); if (!document.hidden) load(); }, 60000); return () => clearInterval(timer); }, []);
  const tracked = new Set(groups.flatMap(g => g.items.map(i => i.symbol)));
  const real = data?.analyses.filter(a => a.close !== null && tracked.has(a.symbol)) ?? [];
  const comparable = real.filter(a => a.status === "ready" && a.change_pct !== null);
  const rising = comparable.filter(a => a.change_pct! > 0).length;
  const falling = comparable.filter(a => a.change_pct! < 0).length;
  const signals = real.filter(a => a.status === "ready").reduce((sum, a) => sum + a.signals.length, 0);
  const ready = real.filter(a => a.status === "ready").length;
  const coverage = tracked.size ? ready / tracked.size : 0;
  const kinds = [
    { label: "趋势", Icon: TrendingUp, count: real.filter(a => a.status === "ready").flatMap(a => a.signals).filter(s => ["breakout", "reversal", "momentum"].includes(s.kind)).length },
    { label: "量能", Icon: ChartNoAxesCombined, count: real.filter(a => a.status === "ready").flatMap(a => a.signals).filter(s => s.kind === "volume").length },
    { label: "风险", Icon: TriangleAlert, count: real.filter(a => a.status === "ready").flatMap(a => a.signals).filter(s => ["volatility", "support_break"].includes(s.kind)).length },
  ];
  const idx = market?.indices.find(i => i.code === indexCode) ?? market?.indices[0];
  const shanghai = new Intl.DateTimeFormat("en-GB", { timeZone: "Asia/Shanghai", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(clock);
  const minutes = Number(shanghai.slice(0, 2)) * 60 + Number(shanghai.slice(3));
  const today = new Intl.DateTimeFormat("en-CA", { timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit" }).format(clock);
  const calendarKnown = market?.as_of === today;
  const session = !calendarKnown ? "交易状态待确认" : !market?.is_trading_day ? "休市日" : minutes < 570 ? "开盘前" : minutes <= 690 ? "交易时段" : minutes < 780 ? "午间休市" : minutes < 900 ? "交易时段" : "已收盘";
  return <section className="watch-overview" aria-label="样本市场概览">
    <div className="watch-overview-head"><span><Activity size={14} /> A 股工作台 <span className="watch-tag">{session}</span></span><span className="muted">日线监控 · 最近应完成交易日 {data?.expected_session || "—"}</span></div>
    <div className="watch-metrics">
      <article className="metric-index"><label className="watch-index-label"><ChartNoAxesCombined size={16} aria-hidden="true" />基准指数<select aria-label="基准指数" value={idx?.code ?? indexCode} onChange={e => setIndexCode(e.target.value)}>{market?.indices.map(i => <option key={i.code} value={i.code}>{i.name}</option>)}</select></label>
        <div className="metric-value"><strong className="mono">{market?.is_demo ? "—" : num(idx?.close)}</strong><span className={`metric-direction ${!market?.is_demo ? changeClass(idx?.change_pct) : "muted"}`} aria-label="指数涨跌方向">{!market?.is_demo && idx && idx.change_pct !== 0 ? idx.change_pct > 0 ? <TrendingUp size={32} /> : <TrendingDown size={32} /> : <ChartNoAxesCombined size={32} />}</span></div><span>{!market?.is_demo && idx ? <Change value={idx.change_pct} /> : "暂无真实指数"}</span><small className="muted">{marketError || `${market?.source || "等待数据"} · ${market?.as_of || "—"}`}{marketError ? <button className="text-btn" onClick={load}>重试</button> : null}</small></article>
      <article className="metric-breadth"><span className="metric-label"><Waves size={16} aria-hidden="true" />样本涨跌</span><div className="metric-value"><strong className="mono">{data ? rising : "—"}<em> / {tracked.size}</em></strong><span className="metric-caption">上涨 / 全部样本</span></div>
        <div className="breadth-visual"><div className="breadth-track" role="img" aria-label={`样本分布：上涨 ${rising}，下跌 ${falling}，平盘 ${comparable.length - rising - falling}，不可比 ${tracked.size - comparable.length}`}>
          <span className="breadth-up" style={{ flex: rising }} /><span className="breadth-down" style={{ flex: falling }} /><span className="breadth-flat" style={{ flex: comparable.length - rising - falling }} /><span className="breadth-unknown" style={{ flex: tracked.size - comparable.length || (tracked.size ? 0 : 1) }} />
        </div><div className="breadth-legend"><span className="is-up">↑ {rising}</span><span className="is-down">↓ {falling}</span><span className="muted">平 {comparable.length - rising - falling}</span><span className="muted">待补 {tracked.size - comparable.length}</span></div></div><small className="muted">同日可比 {comparable.length} 只 · 跨分组去重</small></article>
      <article className="metric-signals"><span className="metric-label"><Radar size={16} aria-hidden="true" />技术信号</span><div className="metric-value"><strong className="mono">{data ? signals : "—"}<em> 条</em></strong><Zap className="metric-symbol" size={28} aria-hidden="true" /></div><div className="signal-categories">{kinds.map(({ label, Icon, count }) => <span key={label}><Icon size={14} aria-hidden="true" />{label}<b className="mono">{count}</b></span>)}</div><small className="muted">仅统计最新真实日线 · 不含过期数据</small></article>
      <article className="metric-coverage"><span className="metric-label"><Database size={16} aria-hidden="true" />数据就绪</span><div className="coverage-content"><div><strong className="mono">{data ? ready : "—"}<em> / {tracked.size}</em></strong><span className="coverage-status"><ShieldCheck size={14} aria-hidden="true" />{!data ? "等待数据" : !tracked.size ? "尚无样本" : ready === tracked.size ? "日线已覆盖" : `${tracked.size - ready} 只待更新`}</span></div><svg className="coverage-ring" viewBox="0 0 64 64" role="img" aria-label={`最新真实日线覆盖率 ${Math.round(coverage * 100)}%`}><circle className="coverage-ring-track" cx="32" cy="32" r="26" /><circle className="coverage-ring-value" cx="32" cy="32" r="26" strokeDasharray={`${coverage * 163.36} 163.36`} /><text x="32" y="36" textAnchor="middle">{data ? `${Math.round(coverage * 100)}%` : "—"}</text></svg></div><small className="muted">分析更新 {data ? new Date(data.generated_at).toLocaleTimeString("zh-CN", { hour12: false }) : "—"}</small></article>
    </div>
    {error ? <p className="inline-notice" role="alert">分析更新失败：{error}，保留上次结果。<button className="text-btn" onClick={onRetry}>重试</button></p> : null}
  </section>;
}

export function WatchSignals({ analyses, onInspect }: { analyses: WatchAnalysis[]; onInspect: (symbol: string) => void }) {
  const [all, setAll] = useState(false);
  const [direction, setDirection] = useState("all");
  const signals = analyses.filter(a => a.status === "ready").flatMap(a => a.signals.map(s => ({ a, s })));
  const filtered = signals.filter(({ s }) => direction === "all" || signalTone(s.kind) === direction);
  return <section className="panel watch-signals" aria-label="AlphaK 量化信号">
    <div className="panel-head"><div><span className="eyebrow mono">ALPHAK / DAILY RULES v1</span><h2>量化信号 <span className="muted">{signals.length}</span></h2></div><span className="watch-tag">日线 · 非买卖指令</span></div>
    <div className="watch-filter-tabs signal-filters" role="group" aria-label="量化信号方向筛选">
      {[
        { value: "all", label: "全部", Icon: LayoutGrid },
        { value: "bullish", label: "看多", Icon: TrendingUp },
        { value: "bearish", label: "看空", Icon: TrendingDown },
        { value: "risk", label: "风险", Icon: TriangleAlert },
        { value: "neutral", label: "中性", Icon: Minus },
      ].map(({ value, label, Icon }) => {
        const count = value === "all" ? signals.length : signals.filter(({ s }) => signalTone(s.kind) === value).length;
        return <button key={value} className={`btn btn-ghost signal-filter-button signal-tone-${value} ${direction === value ? "is-active" : ""}`} aria-label={`${label}信号 ${count} 条`} title={`${label} · ${count} 条信号`} aria-pressed={direction === value} onClick={() => { setDirection(value); setAll(false); }}><Icon size={16} aria-hidden="true" /><span className="mono" aria-hidden="true">{count}</span></button>;
      })}
    </div>
    {filtered.length ? <div className="watch-signal-grid">{(all ? filtered : filtered.slice(0, 5)).map(({ a, s }) => <article className="watch-signal" key={`${a.symbol}-${s.kind}`}>
      <button className="watch-signal-heading" onClick={() => onInspect(a.symbol)}><strong>{a.name}</strong><span className="mono muted">{a.symbol}</span><ArrowUpRight size={14} /></button>
      <h3><SignalTitle kind={s.kind} title={s.title} /></h3><p>{s.evidence}</p><dl><dt>失效条件</dt><dd>{s.invalidation}</dd><dt>主要风险</dt><dd>{s.risk}</dd></dl><footer><span className="mono">日线 {a.as_of}</span><span>{a.source}</span></footer>
    </article>)}</div> : <div className="watch-empty-inline"><Bell size={17} /><div><strong>{signals.length ? "当前没有符合该方向的信号" : "当前没有可报告的新鲜日线信号"}</strong><p>{signals.length ? "可切换其他方向或查看全部信号。" : "仅计算样本范围；需要连续真实复权日线，数据不足时不会产生分数或信号。"}</p></div></div>}
    {filtered.length > 5 ? <button className="btn btn-ghost" onClick={() => setAll(v => !v)}>{all ? "收起" : `查看全部 ${filtered.length} 条`}<ChevronRight size={14} /></button> : null}
    <details className="watch-method"><summary>查看规则口径与评分定义</summary><p>规则符合分 v1：收盘高于 MA20、MA20 高于 MA60、20 日收益为正、成交量 / 前 20 日同源均量 ≥ 1.5，每项 25 分；至少需要 60 根连续有效复权日线。分数不是胜率或收益预测，尚未回测。</p><p>突破：收盘高于前 20 日最高价（不含当日）；反转观察：由下向上穿越 MA20；动量：5 日收益 ≥ 5%；放量：前 20 日均量的 2 倍；高波动：20 日年化历史波动率 ≥ 40%。全部基于已完成交易日，历史绩效未验证。估值、财报事件及自定义策略触发暂未接入。</p></details>
  </section>;
}
