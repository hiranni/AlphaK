/* Hallmark · pre-emit critique: P4 H4 E4 S5 R5 V4 */
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { ColorType, LineSeries, LineType, createChart, type Time } from "lightweight-charts";
import { api, type BreadthPoint } from "../api";
import { CompactStatus } from "../components/CompactStatus";
import { Empty, ErrorState, Loading } from "../components/States";

function ratio(p: BreadthPoint) {
  const total=p.advance+p.decline;
  return total>0 ? p.advance/total*100 : null;
}
function ComparisonChart({sample,market,intraday}: {sample:BreadthPoint[];market:BreadthPoint[];intraday:boolean}) {
  const container=useRef<HTMLDivElement>(null);
  const [theme,setTheme]=useState(0);
  useEffect(()=>{const refresh=()=>setTheme(v=>v+1);window.addEventListener("alphak-theme-change",refresh);return()=>window.removeEventListener("alphak-theme-change",refresh);},[]);
  useEffect(()=>{
    if(!container.current) return;
    const styles=getComputedStyle(document.documentElement);
    const token=(name:string)=>styles.getPropertyValue(name).trim();
    const chart=createChart(container.current,{width:container.current.clientWidth,height:420,layout:{background:{type:ColorType.Solid,color:token("--chart-paper")},textColor:token("--chart-ink"),attributionLogo:false},grid:{vertLines:{color:token("--chart-line")},horzLines:{color:token("--chart-line")}},timeScale:{borderColor:token("--chart-line"),timeVisible:intraday,secondsVisible:false},rightPriceScale:{borderColor:token("--chart-line")}});
    for(const [points,color,title] of [[sample,token("--chart-gold"),"样本"],[market,token("--chart-jade"),"大盘"]] as const){
      const series=chart.addSeries(LineSeries,{color,title,lineWidth:2,lineType:LineType.Curved,priceFormat:{type:"custom",formatter:(value:number)=>`${value.toFixed(1)}%`},priceLineVisible:false,pointMarkersVisible:true,pointMarkersRadius:points.length===1?6:3});
      series.setData(points.filter(p=>ratio(p)!==null).map(p=>({time:(intraday?Number(p.date):p.date) as Time,value:ratio(p)!})));
    }
    chart.timeScale().fitContent();
    const observer=new ResizeObserver(entries=>chart.applyOptions({width:entries[0].contentRect.width}));observer.observe(container.current);
    return()=>{observer.disconnect();chart.remove();};
  },[sample,market,theme,intraday]);
  return <div ref={container} className="breadth-history-chart" aria-label="样本与大盘每日上涨占比曲线" role="img" />;
}

export function BreadthDetail(){
  const navigate=useNavigate();
  const [data,setData]=useState<Awaited<ReturnType<typeof api.breadthHistory>>|null>(null);
  const [error,setError]=useState("");
  const [period,setPeriod]=useState("daily");
  const [showSample,setShowSample]=useState(true);
  const [showMarket,setShowMarket]=useState(true);
  const [membersOpen,setMembersOpen]=useState(new URLSearchParams(window.location.search).has("members"));
  const intraday=period==="30min"||period==="1h";
  const dateLabel=(date:string)=>intraday?new Date(Number(date)*1000).toLocaleString("zh-CN",{timeZone:"Asia/Shanghai",month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit",hour12:false}):date;
  const [retry,setRetry]=useState(0);
  useEffect(()=>{
    let active=true,busy=false;
    const load=async()=>{if(busy||document.hidden)return;busy=true;try{const next=await api.breadthHistory(period);if(active){setData(next);setError("");}}catch(e){if(active)setError(e instanceof Error?e.message:"更新失败");}finally{busy=false;}};
    void load();const timer=window.setInterval(load,30000);return()=>{active=false;window.clearInterval(timer);};
  },[retry,period]);
  if(!data&&error)return <ErrorState message={error} onRetry={()=>setRetry(v=>v+1)} />;
  if(!data)return <Loading label="正在读取涨跌分布历史" />;
  const dates=[...new Set([...data.sample,...data.market].map(p=>p.date))].sort().slice(intraday?-240:-120);
  const sample=data.sample.filter(p=>dates.includes(p.date)),market=data.market.filter(p=>dates.includes(p.date));
  const latestSample=sample.at(-1),latestMarket=market.at(-1);
  const sufficient=(p:BreadthPoint)=>!intraday || (data.members.length>0 && p.advance+p.decline+p.flat>=data.members.length*0.8);
  const partialSample=intraday&&sample.some(p=>!sufficient(p));
  const matching=sample.filter(p=>sufficient(p)&&market.some(m=>m.date===p.date));
  const lastCommon=matching.at(-1),commonMarket=lastCommon?market.find(p=>p.date===lastCommon.date):null;
  const difference=lastCommon&&commonMarket&&ratio(lastCommon)!==null&&ratio(commonMarket)!==null?ratio(lastCommon)!-ratio(commonMarket)!:null;
  return <div className="page stock-page breadth-detail-page"><button className="back-link" onClick={()=>navigate("/")}><ArrowLeft size={14}/> 市场行情</button>
    <header className="stock-head"><div><span className="eyebrow mono">MARKET BREADTH</span><h1 className="display stock-title">大盘与样本涨跌对比</h1><div className="stock-sub"><span className="muted">涨跌股票中的上涨占比 · 同一时间桶对齐</span></div></div><div className="stock-actions"><CompactStatus messages={[error]}/></div></header>
    <section className="stock-stats">{[["样本上涨占比",latestSample],["大盘上涨占比",latestMarket]].map(([label,p])=>{const point=p as BreadthPoint|undefined;return <div key={label as string}><span>{label as string}</span><strong className="mono">{point&&ratio(point)!==null?`${ratio(point)!.toFixed(1)}%`:"—"}</strong><small className="muted">{point?dateLabel(point.date):"暂无观测"}</small></div>;})}<div><span>最近同桶差异</span><strong className="mono">{difference===null?"—":`${difference>0?"+":""}${difference.toFixed(1)} pp`}</strong><small className="muted">{lastCommon?dateLabel(lastCommon.date):"暂无共同时间"}</small></div><div><span>同桶对照</span><strong className="mono">{matching.length} 组</strong></div></section>
    {membersOpen?<section className="panel breadth-members"><div className="panel-head"><h2>行情样本成分 · {data.members.length} 只</h2><button className="text-btn" onClick={()=>setMembersOpen(false)}>收起</button></div><p className="panel-note">市场分布的本地行情样本；全部样本工作台展示同一批股票，分组用于分类研究。</p><div className="breadth-member-list">{data.members.map(m=><button key={m.symbol} className="text-btn" onClick={()=>navigate(`/stock/${m.symbol}`)}><b>{m.name}</b><span className="mono muted">{m.symbol}</span></button>)}</div></section>:null}
    <div className="stock-grid"><section className="panel chart-panel"><div className="panel-head chart-head"><div><span className="eyebrow mono">ADVANCING / TOTAL</span><h2>上涨占比走势</h2></div><div className="segmented" aria-label="曲线粒度">{[["30min","30min"],["1h","1h"],["daily","日"],["weekly","周"],["monthly","月"]].map(([value,label])=><button key={value} className={value===period?"is-active":""} onClick={()=>{if(value!==period){setData(null);setPeriod(value);}}}>{label}</button>)}</div></div>
      <div className="breadth-series-controls"><button className={`btn btn-ghost ${showMarket?"is-active":""}`} aria-pressed={showMarket} onClick={()=>setShowMarket(v=>!v)}><i className="series-swatch market"/>大盘 <span className="muted">{market.length<2?`${market.length} 点 · 暂无连续历史`:""}</span></button><button className={`btn btn-ghost ${showSample?"is-active":""}`} aria-pressed={showSample} onClick={()=>setShowSample(v=>!v)}><i className="series-swatch sample"/>样本{partialSample?<span className="muted">局部缓存 · {latestSample?latestSample.advance+latestSample.decline+latestSample.flat:0}/{data.members.length} 只</span>:null}</button><button className="text-btn" onClick={()=>setMembersOpen(v=>!v)}>查看行情样本 · {data.members.length} 只</button></div>
      {!showMarket&&!showSample?<Empty title="选择大盘或样本以显示曲线"/>:sample.length||market.length?<ComparisonChart sample={showSample?sample:[]} market={showMarket?market:[]} intraday={intraday}/>:<Empty title="暂无可用历史" note="真实历史观测积累后显示曲线。"/>}
      <div className="chart-legend"><span><i className="legend-dot ma5"/>样本</span><span><i className="legend-dot ma20"/>大盘</span><span className="muted">上涨 /（上涨 + 下跌）· 平盘不参与曲线比率</span></div>
      <p className="panel-footnote">{intraday?"分时样本取本地已缓存分时行情，与前收比较；仅有实际缓存的股票参与，覆盖数量可能小于日线样本。大盘取数据源真实发布时间，收盘统计不会展开成盘中曲线。":"大盘历史优先取 AxData 通达信沪深聚合家数；该源不提供平盘数，因此两条曲线统一使用上涨 /（上涨 + 下跌）。日线样本按本地真实收盘价与前收比较；周、月取周期内最后一次真实观测，不累加每日家数。"} {partialSample?" 当前分时仅覆盖局部股票，不能代表完整样本；覆盖不足 80% 的点不计算大盘差异。":""} 同一时间桶取最后一次观测；不同来源不保证同一时刻，缺失不回填。曲线采用平滑显示，圆点才是真实观测值。</p>
      <div className="breadth-availability">{[["大盘",market],["样本",sample]].map(([name,points])=><span key={name as string}>{name as string}：{(points as BreadthPoint[]).length} 个观测点{(points as BreadthPoint[]).length<2?" · 不足以形成曲线":""}</span>)}</div>
    </section><section className="panel"><div className="panel-head"><h2>按时间核对</h2><span className="panel-note">缺失显示 —</span></div><div className="table-wrap"><table className="data-table"><thead><tr><th>日期</th><th className="num">样本上涨占比 / 覆盖</th><th className="num">大盘上涨占比</th><th className="num">差异 / pp</th></tr></thead><tbody>{[...dates].reverse().map(date=>{const s=sample.find(p=>p.date===date),m=market.find(p=>p.date===date),a=s?ratio(s):null,b=m?ratio(m):null;return <tr key={date}><td className="mono">{dateLabel(date)}</td><td className="num mono">{a===null?"—":`${a.toFixed(1)}%`}{s?<span className="cell-sub">{s.advance+s.decline+s.flat} 只</span>:null}</td><td className="num mono">{b===null?"—":`${b.toFixed(1)}%`}{m?.phase==="intraday"?" · 盘中":""}</td><td className="num mono">{a!==null&&b!==null&&s&&sufficient(s)?(a-b).toFixed(1):"—"}</td></tr>;})}</tbody></table></div></section></div>
  </div>;
}
