# Quant Execution

## Purpose

让用户在人工限定的A股股票池内执行可解释、可回溯的量化规则；由同一决策语义支持历史研究和模拟运行，并以平台控制的风控、成交和账本防止虚构收益或重复交易。

## ADDED Requirements

### Requirement: Immutable scoped run
系统 SHALL 将每次运行绑定不可变策略、人工1–8只股票池、输入manifest、代码、费用、日历和执行规则版本。系统 MUST 拒绝未来数据、未知策略版本和超范围标的，并披露事后重建和缺失能力。

#### Scenario: Future or oversized input
- **WHEN** 提交9只股票或判断时点后才可用的数据
- **THEN** 运行被拒并返回具体原因，不创建模拟订单

#### Scenario: Reproduce a run
- **WHEN** 相同冻结输入和版本重新执行
- **THEN** 规范化决策、成交、净值和指标相同，运行时间和随机标识不参与经济结果比较

### Requirement: Versioned decisions and target positions
系统 SHALL 分离状态、触发事件、目标仓位、订单和成交；每个目标包含约束前后数量、理由、数据覆盖和状态变化。发布后的策略不可覆盖。未知输入不得被当成退出事实或有效买入条件。

#### Scenario: Repeated event
- **WHEN** 同一判断被重试或进程重启
- **THEN** 已消费候选、冷却和减仓锁恢复，同一事件不重复买入或连续减半

#### Scenario: Missing sentiment
- **WHEN** T1缺少同口径历史宽度
- **THEN** T1返回等待原因，不静默按T0规则运行

### Requirement: Platform validation and risk authority
系统 SHALL 对包括rule Agent在内的决策先做schema再做业务校验，强制资金、持仓、在途订单、风险预算和kill switch。Agent MUST NOT 直接写订单或账本；Agent版本绑定实例而非策略。

#### Scenario: Competing actions
- **WHEN** 同日同标的同时满足买入和退出
- **THEN** 平台按冻结优先级合并为唯一最终目标，退出优先，保存全部原因

### Requirement: Conservative simulated matching
系统 SHALL 收盘后判断、最早下一交易日尝试成交，使用原始价、版本化费用、交易单位、可卖数量、停牌和涨跌停约束。缺执行证据不得伪造成交；前向运行不得用事后日线开盘价补造当时未观察的成交。

#### Scenario: Unavailable execution
- **WHEN** 次日停牌、不可成交涨跌停或T+1限制卖出
- **THEN** 订单记录拒绝或待执行原因和剩余量，现金持仓不被当作已成交更新

### Requirement: Durable isolated ledger
系统 SHALL 使用实例独立子账本；订单、成交、费用、现金、持仓及净值关联决策和数据版本。订单状态变化与资金冻结必须原子提交，账户分配不得超额。回放只读，缓存清理不得删除账本。

#### Scenario: Crash after fill
- **WHEN** 成交入账后进程中断并重启
- **THEN** 成交幂等恢复，现金和持仓仅变更一次，账户分配守恒

#### Scenario: Explainability expiration
- **WHEN** 解释缓存过期
- **THEN** 回放返回CACHE_EXPIRED，交易和净值记录保留
