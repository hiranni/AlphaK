import { ArrowRight, FlaskConical, Layers3, Wallet } from "lucide-react";
import { Link } from "react-router-dom";

// Hallmark · pre-emit critique: P5 H5 E4 S5 R5 V4 · existing terminal system.
export function TradingWorkspace({ mode }: { mode: "quant" | "trading" }) {
  const quant = mode === "quant";
  return <div className="page strategy-entry">
    <header className="page-head"><div className="page-head-copy"><h1 className="display">{quant ? "量化" : "实盘"}</h1><span className="muted">{quant ? "策略研究与验证" : "模拟资金 · 模拟成交"}</span></div><span className="watch-tag">{quant ? "策略工作区" : "模拟运行环境"}</span></header>
    <section className="panel strategy-entry-main">
      <div className="strategy-entry-intro">{quant ? <FlaskConical size={26} aria-hidden="true" /> : <Wallet size={26} aria-hidden="true" />}<div><h2>{quant ? "尚未配置策略" : "尚未创建账户"}</h2><p>{quant ? "在这里定义股票池、交易条件与风控规则，验证后交由实盘中的策略实例运行。" : "在这里管理模拟资金、策略实例、持仓与订单。一个账户可运行多个策略，各策略独立核算。"}</p></div></div>
      <div className="strategy-entry-sections">
        {(quant ? [
          ["策略定义", "名称与版本、股票池、买卖条件、仓位和风险限制"],
          ["历史验证", "回测收益、回撤、交易成本与信号解释"],
          ["部署运行", "选择账户、分配资金，创建独立策略实例"],
        ] : [
          ["账户与资金", "总资产、未分配现金和各策略资金分配"],
          ["策略与持仓", "运行状态、独立子账本、净值和持仓明细"],
          ["订单与记录", "模拟撮合、撤单、拒单原因与操作日志"],
        ]).map(([title, description]) => <div key={title}><h3>{title}<span className="muted">待建设</span></h3><p>{description}</p></div>)}
      </div>
      <footer className="strategy-entry-foot"><span><Layers3 size={14} aria-hidden="true" />单策略最多 8 只股票 · 日频信号于下一交易日撮合</span><Link className="text-link" to={quant ? "/trading" : "/quant"}>{quant ? "查看实盘" : "查看量化"}<ArrowRight size={14} /></Link></footer>
    </section>
  </div>;
}
