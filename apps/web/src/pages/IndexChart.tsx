/* Hallmark · pre-emit critique: P4 H4 E4 S5 R5 V4 */
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { api } from "../api";
import type { BarsResponse, IndexDetail } from "../types";
import { KLineChart } from "../components/KLineChart";
import { Change } from "../components/Change";
import { CompactStatus } from "../components/CompactStatus";
import { Empty, ErrorState, Loading } from "../components/States";
import { money, num } from "../format";

export function IndexChart() {
  const { code = "" } = useParams();
  const navigate = useNavigate();
  const [quote, setQuote] = useState<IndexDetail | null>(null);
  const [bars, setBars] = useState<BarsResponse | null>(null);
  const [period, setPeriod] = useState("daily");
  const [indicator, setIndicator] = useState<"macd" | "kdj" | "none">("macd");
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let active = true;
    let busy = false;
    setBars(null); setQuote(null); setError("");
    const load = async () => {
      if (busy || document.hidden) return;
      busy = true;
      try {
        const [q,b] = await Promise.all([api.index(code), api.indexBars(code,period)]);
        if (active) { setQuote(q); setBars(b); setError(""); }
      } catch (e) { if (active) setError(e instanceof Error ? e.message : "指数更新失败"); }
      finally { busy = false; }
    };
    void load(); const timer = window.setInterval(load,60000);
    return () => { active=false; window.clearInterval(timer); };
  },[code,period,retry]);
  if (!quote && error) return <ErrorState message={error} onRetry={() => setRetry(v=>v+1)} />;
  if (!quote) return <Loading label="正在读取指数行情" />;
  const last = bars?.bars.at(-1);
  return <div className="page stock-page index-chart-page">
    <button className="back-link" onClick={() => navigate("/")}><ArrowLeft size={14} /> 市场行情</button>
    <header className="stock-head"><div><span className="eyebrow mono">INDEX · {code}</span><h1 className="display stock-title">{quote.name}</h1><div className="stock-sub"><span className="mono muted">数据截至 {quote.as_of}</span></div></div><div className="stock-price-block"><strong className="stock-price mono">{num(quote.close)}</strong><Change value={quote.change_pct} /><Change value={quote.change_points} points /></div><div className="stock-actions"><CompactStatus messages={[error]} /></div></header>
    <section className="stock-stats"><div><span>{period==="monthly"?"本月开盘":"开盘"}</span><strong className="mono">{num(last?.open)}</strong></div><div><span>{period==="monthly"?"月内最高":"最高"}</span><strong className="mono">{num(last?.high)}</strong></div><div><span>{period==="monthly"?"月内最低":"最低"}</span><strong className="mono">{num(last?.low)}</strong></div><div><span>成交额</span><strong className="mono">{quote.amount_known ? money(quote.amount) : "—"}</strong></div></section>
    <div className="stock-grid"><section className="panel chart-panel"><div className="panel-head chart-head"><div><span className="eyebrow mono">PRICE / VOLUME</span><h2>指数走势</h2></div><div className="chart-controls"><div className="segmented" aria-label="指数周期">{[["daily","日K"],["monthly","月K"]].map(([value,label])=><button key={value} className={period===value?"is-active":""} onClick={()=>setPeriod(value)}>{label}</button>)}</div><div className="segmented" aria-label="技术指标">{(["macd","kdj","none"] as const).map(v=><button key={v} className={indicator===v?"is-active":""} onClick={()=>setIndicator(v)}>{v==="none"?"关闭":v.toUpperCase()}</button>)}</div></div></div>{bars ? bars.bars.length ? <KLineChart bars={bars.bars} indicator={indicator} /> : <Empty title="暂无真实指数 K 线" note="等待历史行情同步后显示。" /> : <Loading label="正在绘制指数 K 线" />}<div className="chart-legend"><span className="mono"><i className="legend-dot ma5" />MA5</span><span className="mono"><i className="legend-dot ma20" />MA20</span><span className="mono"><i className="legend-dot ma60" />MA60</span><span className="muted">指数不复权</span></div></section></div>
  </div>;
}
