---
spec_id: STRAT-0001
title: 策略名称
version: 1.0.0
status: draft
owner: ""
created_at: 2026-10-21
updated_at: 2026-10-21
strategy_spec: examples/strategy.example.yaml
strategy_instance: examples/strategy-instance.example.yaml
agent_profile: examples/agent-profile.example.yaml
universe_max: 8
execution_mode: advisory
cache_granularity: quarterly
retention_periods: 8
---

# 策略分析：{{title}}

> 本文件既是人类可读的策略说明，也是 Agent 的执行契约。所有规范性描述必须能映射到
> `strategy.schema.json` 字段或 `docs/specs/README.md` 中的 invariant ID。无法映射的描述只能作为背景说明。

## 0. Agent 阅读摘要

用不超过 10 行说明策略的核心，Agent 读完这一节应能知道「做什么、在哪做、什么不做」。

| 项目 | 内容 |
| --- | --- |
| 策略 ID | `STRAT-0001` |
| 目标 | 相对基准的超额收益 / 绝对收益 / 回撤控制 |
| 股票池 | 最多 8 只，列出代码或引用 `strategy_spec` |
| 信号 | 1–3 句话概括 |
| 执行模式 | `advisory` / `auto` / `shadow` |
| 策略特征 | `trend` / `daily` / `medium` / 20 天 / 年化换手 30% |
| 可解释缓存 | `quarterly`（默认），保留 8 个季度 |
| 最大持仓 | ≤ 8 |
| 停止条件 | 最大回撤、连续失败、数据中断等 |
| 明确不做 | 全市场扫描、池外交易、实盘下单 |

## 1. 假设与边界

| 编号 | 假设 | 若假设不成立的影响 |
| --- | --- | --- |
| A-01 | 使用日频前复权行情，收盘后生成信号 | 信号时点与撮合价格需调整 |
| A-02 | 财务数据按公告日可用 | 回测可能出现未来数据 |
| A-03 | 交易成本按配置费率计算 | 收益会被高估或低估 |
| A-04 | 策略只操作股票池内标的 | 超出即违反 INV-01/INV-03 |

**边界**：

- 不提供全市场扫描、排名或条件选股。
- 不接券商、不产生真实委托。
- 不构成投资建议。

## 2. 股票池（≤8）

| 序号 | 代码 | 名称 | 行业 | 加入理由 | 备注 |
| --- | --- | --- | --- | --- | --- |
| 1 | `600519.SH` | 示例 | 食品饮料 | 流动性好 | — |
| 2 | `000858.SZ` | 示例 | 食品饮料 | 流动性好 | — |
| 3 |  |  |  |  |  |
| 4 |  |  |  |  |  |
| 5 |  |  |  |  |  |
| 6 |  |  |  |  |  |
| 7 |  |  |  |  |  |
| 8 |  |  |  |  |  |

校验：

- [ ] 1–8 只，代码唯一，带交易所后缀。
- [ ] `max_symbols: 8`。
- [ ] 不使用指数成分、行业全量或动态池。

## 3. 数据与信号

| 项目 | 值 |
| --- | --- |
| 频率 | `daily` |
| 回看窗口 | 250 个交易日 |
| 字段 | `open, high, low, close, volume, amount, pe_ttm, roe` |
| point-in-time | `true` |

信号表：

| 名称 | 表达式 | 输入 | 输出 | 说明 |
| --- | --- | --- | --- | --- |
| `ma20` | `SMA(close, 20)` | close | number | 20 日均线 |
| `ma60` | `SMA(close, 60)` | close | number | 60 日均线 |
| `trend_up` | `close > ma20 AND ma20 > ma60` | close, ma20, ma60 | boolean | 趋势向上 |

表达式必须符合 `docs/specs/README.md` 的 EBNF；不得使用循环、网络、随机数或自定义函数。

## 4. 入场、出场与仓位

| 规则 | 条件 | 动作 | 权重 | 优先级 |
| --- | --- | --- | --- | --- |
| 入场 1 | `trend_up AND CROSS(close, ma20)` | buy | 0.125 | 80 |
| 出场 1 | `close < ma20` | sell | 1.0 | 90 |
| 出场 2 | `close < REF(close, 1) * 0.92` | reduce | 0.5 | 95 |

仓位：

| 项目 | 值 |
| --- | --- |
| 方法 | `equal_weight` |
| 最大持仓数 | 8 |
| 单票最大权重 | 0.20 |
| 现金缓冲 | 0.05 |

## 5. 风险与约束

| 项目 | 阈值 | 触发后行为 |
| --- | --- | --- |
| 单票止损 | 8% | 卖出该标的 |
| 最大回撤 | 15% | 暂停策略，等待人工复核 |
| 单日换手 | 30% | 拒绝超出部分 |
| 单笔订单金额 | 50,000 CNY | 拒绝或拆分 |
| 交易时段 | 09:30–15:00 Asia/Shanghai | 时段外不下单 |

平台强制项（Agent 不可覆盖）：

- [ ] 股票池 ≤8、持仓 ≤8。
- [ ] 池外标的订单直接拒绝。
- [ ] 无实盘接口、无券商凭据。
- [ ] Kill switch 可随时暂停。
- [ ] 所有决策经过 schema + 业务校验。

### 5.1 策略特征（characteristics）

| 字段 | 值 | 口径 |
| --- | --- | --- |
| `style` | `trend` | 趋势 / 动量 / 均值回归 / 价值 / 质量 / 事件 / 多因子 |
| `horizon` | `daily` | `intraday` / `daily` / `weekly` / `monthly` / `quarterly` |
| `risk_level` | `medium` | `low` / `medium` / `high` |
| `typical_holding_days` | 20 | 预期持仓交易日中位数 |
| `expected_turnover_pct` | 0.30 | 年化换手，小数 |
| `market_regime` | `trending, volatile` | 适用市场状态 |
| `tags` | `ma, large-cap` | 便于比较的标签 |

### 5.2 可解释与回放（explainability）

| 项目 | 值 |
| --- | --- |
| 缓存粒度 | `quarterly`（默认；可选 `weekly` / `monthly` / `yearly`） |
| 保留期 | 8 个季度（按所选粒度计数） |
| 周期键示例 | `2026-Q4` |
| 包含内容 | `context_snapshot`, `signal_values`, `rule_trace`, `agent_prompt`, `agent_response`, `tool_calls`, `validation_trace`, `position_snapshot`, `order_links` |
| 回放模式 | `decision` / `period` |
| 脱敏 | `true` |
| 固定周期 | 无 / 列出 `pinned_periods` |
| 清理规则 | 到期只删除可解释缓存，不删除 NAV、成交、持仓 |

## 6. 回测与虚拟盘

| 指标 | 回测 | 虚拟盘 | 基准 |
| --- | --- | --- | --- |
| 年化收益 |  |  |  |
| 最大回撤 |  |  |  |
| 夏普 |  |  |  |
| 胜率 |  |  |  |
| 换手率 |  |  |  |
| 样本天数 |  |  |  |

要求：

- 回测与虚拟盘使用同一套策略定义、撮合规则与费用口径。
- 回测必须记录数据版本与股票池快照；虚拟盘必须记录每次 Agent 决策。
- 回测收益不得直接作为虚拟盘或未来收益的承诺。

## 7. Agent 执行契约

| 项目 | 值 |
| --- | --- |
| 实例配置 | `strategy-instance.yaml` |
| Agent Profile | `agent-trend-llm@1.0.0` |
| 类型 | `llm` / `hybrid` / `rule` |
| 模式 | `advisory` / `auto` / `shadow` |
| 允许工具 | `market.read, fundamentals.read, indicators.compute, portfolio.read, order.propose` |
| 输出 Schema | `agent-decision.schema.json` |
| 失败回退 | `hold` / `skip` / `rule_based` |
| 人工审批 | 需要 / 不需要 |
| 成本上限 | 每日调用次数、token、金额 |

执行流程：

1. 平台构建上下文（池内行情、持仓、现金、约束、数据快照）。
2. Agent 返回 `agent-decision.schema.json` 兼容的 JSON。
3. 平台做 schema 校验与业务校验。
4. `advisory` 进入人工审批；`auto` 在限额内提交虚拟订单；`shadow` 只记录不成交。
5. 记录决策、工具调用、校验结果、订单号与耗时。

## 8. 评估与停止条件

| 条件 | 阈值 | 动作 |
| --- | --- | --- |
| 最大回撤 | 15% | 暂停策略 |
| 连续无效决策 | 3 次 | 暂停 Agent，人工检查 |
| 数据中断 | 1 个交易日 | 不产生新订单 |
| 成本超限 | 日预算 100% | 当日停止 Agent |
| 合规校验失败 | 任意一次 | 拒绝该决策并告警 |

评估维度：

- 收益、回撤、夏普、换手、胜率。
- 决策合规率、无效输出率、回退率、平均延迟、单次成本。
- 与基准、与其他 Agent（同策略/同池）的对比。

## 9. 变更记录

| 版本 | 日期 | 变更 | 影响 |
| --- | --- | --- | --- |
| 1.0.0 | 2026-10-21 | 初始版本 | — |

## 10. Agent 自检清单

- [ ] `strategy.yaml` 通过 `strategy.schema.json`。
- [ ] `strategy-instance.yaml` 通过 `strategy-instance.schema.json`。
- [ ] `agent-profile.yaml` 通过 `agent-profile.schema.json`。
- [ ] 示例决策通过 `agent-decision.schema.json`。
- [ ] 回放缓存示例通过 `explainability.schema.json`。
- [ ] 股票池 1–8 只，持仓上限 ≤8。
- [ ] 所有信号表达式在允许的语法内。
- [ ] 所有订单标的都在池内。
- [ ] 执行模式、回退策略、审批路径明确。
- [ ] 停止条件与告警接收人明确。
- [ ] `characteristics` 完整，数值字段有单位/口径。
- [ ] `explainability.cache_granularity` 与 `retention_periods` 明确，默认季度。
- [ ] 决策轨迹包含上下文、信号、校验、持仓前后与订单关联。
- [ ] 回放只读，过期缓存返回 `CACHE_EXPIRED`，不伪造。
- [ ] 清理只删除可解释缓存，不删除 NAV、成交、持仓。
- [ ] 文档中的数字、单位、时区与 schema 一致。
