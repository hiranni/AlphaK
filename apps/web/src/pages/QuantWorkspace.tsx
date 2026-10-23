import { Activity, ArrowRight, ChartNoAxesCombined, FlaskConical, History, Layers3, ListFilter, Wallet } from "lucide-react";
import { Link, NavLink, Outlet } from "react-router-dom";

export const quantPages = [
  { path: "", label: "量化总览", icon: Activity },
  { path: "strategies", label: "策略中心", icon: Layers3 },
  { path: "factors", label: "因子与选股", icon: ListFilter },
  { path: "backtests", label: "回测中心", icon: History },
  { path: "paper", label: "模拟交易", icon: Wallet },
  { path: "signals", label: "信号中心", icon: ChartNoAxesCombined },
];
export const tradingPages = [
  { path: "", label: "账户总览" }, { path: "strategies", label: "策略运行" },
  { path: "positions", label: "持仓" }, { path: "orders", label: "订单" }, { path: "records", label: "运行记录" },
];
export function StrategyModule({ trading = false }: { trading?: boolean }) {
  const base = trading ? "/trading" : "/quant";
  return <div className="page strategy-module">
    <header className="page-head"><div className="page-head-copy"><h1 className="display">{trading ? "实盘" : "量化"}</h1><span className="muted">{trading ? "模拟资金 · 模拟成交" : "策略研发与验证"}</span></div><span className="watch-tag">{trading ? "模拟运行环境" : "A 股 · 日频"}</span></header>
    <nav className="module-nav" aria-label={trading ? "实盘二级导航" : "量化二级导航"}>{(trading ? tradingPages : quantPages).map(p => <NavLink key={p.path} to={`${base}${p.path ? `/${p.path}` : ""}`} end={!p.path} className={({isActive}) => isActive ? "is-active" : ""}>{p.label}</NavLink>)}</nav>
    <Outlet />
  </div>;
}
const sections: Record<string, { title: string; description: string; fields: [string,string][] }> = {
  strategies: {title:"策略中心",description:"管理策略定义与版本；运行实例单独绑定账户和 Agent。",fields:[['基本资料与版本','名称、ID、A 股市场、周期、作者与说明；修改参数生成新版本，保留历史和回滚记录。'],['交易与仓位规则','入场、退出、调仓频率、单票仓位及组合上限；每策略股票池与持仓最多 8 只。'],['风险与生命周期','止损、回撤停机、流动性限制；研究、待验证、待部署与暂停状态分别管理。']]},
  factors:{title:"因子与选股",description:"在用户指定股票池内比较因子、筛选候选标的；评分是状态，信号是时点事件。",fields:[['因子视图','趋势、动量、成交量、估值与风险分别展示，缺失数据不补成评分。'],['池内筛选','按规则筛选候选股票；策略股票池最多 8 只，不进行全市场扫描或排名。'],['有效性验证','因子相关性、分组未来收益、稳定性和样本外表现需由验证任务产生。']]},
  backtests:{title:"回测中心",description:"固定策略版本、数据时间与成本假设，分别保存训练、验证和样本外结果。",fields:[['区间与数据','股票池、基准、起止日期和日线频率；记录复权、分红、退市与历史成分处理口径。'],['交易与成本','入场、退出、仓位、风控、手续费与滑点；日频收盘信号于下一交易日撮合。'],['结果与复核','净值与基准曲线、年化收益、最大回撤、Sharpe、胜率、盈亏比、换手率、交易记录和参数敏感性。']]},
  paper:{title:"模拟交易",description:"使用实盘模块统一管理模拟账户，量化负责策略研发与部署衔接。",fields:[['部署策略实例','锁定策略版本和参数，选择账户、分配资金，再创建独立子账本。'],['跟踪运行表现','查看模拟净值、收益与回撤；单独记录，不能替代回测或样本外验证结果。'],['检查交易执行','查看信号、风控、模拟订单、持仓与拒单原因，所有运行记录可追溯。']]},
  signals:{title:"信号中心",description:"汇总策略事件、触发依据、失效条件、通知状态与历史记录。",fields:[['信号状态','按新触发、已确认和已失效分类，保留策略版本、标的、周期与触发时间。'],['解释与风险','展示触发规则、数据依据、失效条件和风险说明，评分不作为预测准确率。'],['通知与追踪','通知渠道、冷却时间、确认记录与历史检索；策略信号与自选观察信号分别记录。']]},
};
function PendingBlock({title,children}:{title:string;children:React.ReactNode}) { return <section className="panel quant-pending"><div className="panel-head"><h2>{title}</h2><span className="muted">未接入</span></div><p>{children}</p></section>; }
export function QuantOverview() {
  return <>
    <div className="quant-metrics">{[['策略总数','全部 / 启用 / 暂停'],['今日信号','新触发 / 已确认 / 已失效'],['最近回测','年化收益 / 最大回撤 / Sharpe'],['模拟组合','净值 / 收益率 / 当前回撤'],['数据健康','行情延迟 / 缺失 / 任务失败'],['待处理事项','待验证 / 回测失败 / 风险告警']].map(([title,note])=><div key={title}><span>{title}</span><strong className="mono">—</strong><small>{note}</small><small>未接入</small></div>)}</div>
    <div className="quant-overview-grid"><PendingBlock title="策略表现对比">尚无策略表现记录。最近回测、样本外验证、模拟运行将分别展示与排序。</PendingBlock><PendingBlock title="近期信号">策略事件接入后展示触发时间线、解释与状态。<Link className="text-link" to="/quant/signals">信号中心 <ArrowRight size={13}/></Link></PendingBlock><PendingBlock title="运行异常">策略执行、数据质量与风控告警尚未接入。<Link className="text-link" to="/data">查看现有数据管理 <ArrowRight size={13}/></Link></PendingBlock><PendingBlock title="最近回测">尚无回测任务与结果。<Link className="text-link" to="/quant/backtests">回测中心 <ArrowRight size={13}/></Link></PendingBlock></div>
  </>;
}
export function QuantSection({section}:{section:string}) {
  const s=sections[section];
  return <section className="panel strategy-entry-main"><div className="strategy-entry-intro"><FlaskConical size={24} aria-hidden="true"/><div><h2>{s.title}</h2><p>{s.description}</p></div></div><div className="strategy-entry-sections">{s.fields.map(([title,description])=><div key={title}><h3>{title}<span className="muted">待建设</span></h3><p>{description}</p></div>)}</div>{section==='paper' ? <Link className="btn btn-soft" to="/trading">进入实盘<ArrowRight size={14}/></Link> : <p className="panel-footnote">当前为模块设计入口，配置、任务与执行服务尚未接入。</p>}</section>;
}
export function TradingSection({section}:{section:string}) {
 const title=tradingPages.find(p=>p.path===section)?.label || '账户总览';
 return <PendingBlock title={title}>{section==='strategies'?'尚未部署策略实例。每个实例绑定策略版本、账户和独立子账本。':section==='positions'?'尚无模拟持仓。持仓将按策略子账本分别核算。':section==='orders'?'尚无模拟订单。订单将展示状态、撮合时间和拒单原因。':section==='records'?'尚无运行记录。资金分配、信号校验、风控与撮合均需留痕。':'账户服务尚未接入。创建后将展示总资产、未分配现金和策略资金分配。'}<Link className="text-link" to="/quant/strategies">前往策略中心 <ArrowRight size={13}/></Link></PendingBlock>;
}
