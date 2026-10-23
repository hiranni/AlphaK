import { useEffect, useRef, useState } from "react";
import { ArrowUpRight, BellPlus, X } from "lucide-react";
import { useNavigate } from "react-router-dom";
import type { BarsResponse, WatchAnalysis } from "../types";
import { api } from "../api";
import { num, pct, compact, changeClass } from "../format";
import { SignalTitle } from "./WatchWorkbench";
import { KLineChart } from "./KLineChart";

export function WatchAnalysisPanel({ symbol, analysis, onClose, onAlert }: { symbol: string; analysis?: WatchAnalysis; onClose: () => void; onAlert: (symbol: string) => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  const [bars, setBars] = useState<BarsResponse | null>(null);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const navigate = useNavigate();
  useEffect(() => {
    const dialog = ref.current; const previous = document.activeElement as HTMLElement | null;
    dialog?.showModal();
    return () => { dialog?.close(); previous?.focus(); };
  }, []);
  useEffect(() => {
    let active = true; setError(""); setBars(null);
    api.bars(symbol, "daily", "qfq").then(v => { if (active) setBars(v); }).catch((e: Error) => { if (active) setError(e.message); });
    return () => { active = false; };
  }, [symbol, retry]);
  const a = analysis;
  const chartBars = bars?.bars.filter(b => !!a?.history_start && b.date >= a.history_start && b.date <= a.as_of) || [];
  return <dialog ref={ref} className="watch-dialog watch-analysis-dialog" aria-label="个股快捷分析" onCancel={onClose} onClick={e => { if (e.target === ref.current) { const r = ref.current.getBoundingClientRect(); if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) onClose(); } }}>
    <header className="watch-dialog-head"><div><span className="eyebrow mono">QUICK ANALYSIS / DAILY</span><h2>{a?.name || symbol} <small className="mono muted">{symbol}</small></h2></div><button className="icon-btn" onClick={onClose} aria-label="关闭快捷分析"><X size={18} /></button></header>
    <div className="watch-dialog-body">
      <div className="watch-analysis-price"><strong className={`mono ${changeClass(a?.change_pct)}`}>{num(a?.close)}</strong><span className={changeClass(a?.change_pct)}>{pct(a?.change_pct)}</span><span className="muted">{a?.source || "未取得"} · 日线 {a?.as_of || "—"}</span></div>
      <p className={`watch-data-notice ${a?.status !== "ready" ? "is-warning" : ""}`}>{a?.notice || "分析尚未取得，请返回后重试"}</p>
      <div className="watch-analysis-metrics"><div><span>趋势</span><strong>{a?.trend || "—"}</strong></div><div><span>规则符合分</span><strong>{a?.score === null || a?.score === undefined ? "—" : `${a.score}/100`}</strong></div><div><span>20 日历史年化波动率</span><strong>{num(a?.volatility)}{a?.volatility != null ? "%" : ""}</strong></div><div><span>成交量 / 前 20 日均量</span><strong>{num(a?.volume_ratio)}{a?.volume_ratio != null ? " 倍" : ""}</strong></div></div>
      {a && a.close !== null && a.status !== "demo" ? <>
        {error ? <p role="alert">图表加载失败：{error} <button className="text-btn" onClick={() => setRetry(v => v + 1)}>重试</button></p> : bars ? chartBars.length ? <><KLineChart bars={chartBars} indicator="macd" /><p className="muted">图表截至 {bars.as_of} · {bars.adjust === "qfq" ? "前复权" : "不复权"}。{bars.notice} 上方信号仅使用已完成交易日。</p></> : <p className="muted">暂无日线</p> : <p role="status">正在加载日线图…</p>}
      </> : <p className="muted">缺失或演示日线不展示为真实分析。</p>}
      <dl className="watch-analysis-levels"><div><dt>前 20 日低点（参考支撑）</dt><dd className="mono">{num(a?.support)}</dd></div><div><dt>前 20 日高点（参考阻力）</dt><dd className="mono">{num(a?.resistance)}</dd></div><div><dt>MA20 / MA60</dt><dd className="mono">{num(a?.ma20)} / {num(a?.ma60)}</dd></div><div><dt>成交额（元）</dt><dd className="mono">{compact(a?.amount)}</dd></div></dl>
      <h3>触发依据与风险</h3>
      {a?.signals.length ? a.signals.map(s => <article className="watch-quick-signal" key={s.kind}><strong><SignalTitle kind={s.kind} title={s.title} /></strong><p>{s.evidence}</p><p><span className="muted">失效条件：</span>{s.invalidation}</p><p><span className="muted">风险：</span>{s.risk}</p></article>) : <p className="muted">当前未出现可计算的技术条件，或数据不足。</p>}
      <p className="panel-footnote">日频规则观察，非买卖指令。评分未经回测，不表示预测准确率；参考价位不能保证有效支撑或阻力。历史波动率不表示未来风险上限。</p>
    </div>
    <footer className="watch-dialog-foot"><button className="btn btn-ghost" onClick={() => { onClose(); onAlert(symbol); }}><BellPlus size={14} /> 设置提醒</button><button className="btn btn-solid" onClick={() => { onClose(); navigate(`/stock/${symbol}`); }}>完整个股分析<ArrowUpRight size={14} /></button></footer>
  </dialog>;
}
