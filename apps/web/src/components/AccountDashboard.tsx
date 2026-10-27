/* Hallmark · pre-emit critique: P4 H4 E4 S5 R5 V4 */
import { useState, type ReactNode } from 'react';
import { Activity, BarChart3, CalendarDays, Landmark, Wallet } from 'lucide-react';
import type { PaperStrategy } from '../api';
const money=(v:number)=>`¥${(v/100).toLocaleString('zh-CN',{minimumFractionDigits:2,maximumFractionDigits:2})}`;
export function AccountDashboard({account,configuration}:{account:PaperStrategy;configuration:ReactNode}) {
 const [metric,setMetric]=useState('收益率'),[period,setPeriod]=useState('本月');
 // The current ledger supports initial funding only; never infer returns from research backtests.
 const profit=account.cash_fen-account.initial_cash_fen;
 const rate=account.initial_cash_fen>0?profit/account.initial_cash_fen*100:0;
 return <div className="account-dashboard">
  <section className="account-assets" aria-label="账户资产">
   <div className="account-assets-main"><span className="account-eyebrow"><Wallet size={16}/>总资产（元）</span><strong>{money(account.cash_fen).slice(1)}</strong><div className="account-cash"><span>可用资金 <b>{money(account.cash_fen)}</b></span><span>持仓占比 <b>0.00%</b></span></div></div>
   <div className="account-metrics">
    <div><span><Landmark size={15}/>总市值</span><strong>¥0.00</strong><small>当前无持仓</small></div>
    <div><span><Activity size={15}/>总盈亏</span><strong className={profit>0?'account-positive':profit<0?'account-negative':''}>{money(profit)}</strong><small>累计收益率 {rate.toFixed(2)}%</small></div>
    <div><span><BarChart3 size={15}/>当日参考盈亏</span><strong>—</strong><small>等待前一交易日资产基准</small></div>
    <div><span><CalendarDays size={15}/>本月盈亏</span><strong>—</strong><small>等待月初资产基准</small></div>
   </div>
  </section>
  <div className="account-analysis-layout"><section className="panel account-performance">
   <div className="account-chart-head"><div><h3>账户收益</h3><p>账户表现与上证指数 · 同区间比较</p></div><div className="account-segments" aria-label="收益区间">{['本月','近3月','今年','全部'].map(x=><button key={x} aria-pressed={period===x} onClick={()=>setPeriod(x)}>{x}</button>)}</div></div>
   <div className="account-return-strip"><div><span>{period}收益率</span><strong>—<small>%</small></strong></div><div><span>{period}收益金额</span><strong>—<small>元</small></strong></div><div><span>跑赢上证指数</span><strong>—<small>个百分点</small></strong></div><div className="account-benchmark-note">超额收益 = 账户收益率 − 上证指数涨跌幅<br/>均以所选区间起点为基准</div></div>
   <div className="account-chart-tools"><div className="account-segments" aria-label="收益指标">{['收益率','收益金额','收益K线'].map(x=><button key={x} aria-pressed={metric===x} onClick={()=>setMetric(x)}>{x}</button>)}</div><span className="account-chart-legend"><i/>账户{metric}<i/>上证指数{metric==='收益金额'?'（仅收益率可比）':''}</span></div>
   <div className="account-chart-empty" role="status"><div className="account-chart-grid" aria-hidden="true"/><div><BarChart3 size={32}/><h4>尚无{period}收益{metric==='收益K线'?'K线':'记录'}</h4><p>{metric==='收益K线'?'收益K线需要每日资产估值的开、高、低、收数据。':'账户尚未形成连续资产估值，暂无法计算区间收益及指数对比。'}</p><small>开户日期 {account.created_at.slice(0,10)} · 初始资金 {money(account.initial_cash_fen)}</small></div></div>
   <p className="account-chart-foot">收益以账户资金与持仓估值计算，剔除出入金影响；历史研究报告不计入账户收益。</p>
  </section>{configuration}</div>
 </div>
}
