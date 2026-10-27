# Quant Experiments

## Purpose

让用户自动执行有限、可复现的策略参数实验，比较成本后表现和失败原因；严格隔离训练、验证及未见区间，将研究候选与正在运行的模拟策略区分，允许得到没有改善的结论。

## ADDED Requirements

### Requirement: Bounded reproducible search
系统 SHALL 冻结搜索空间、预算、时段、基准、种子及评价政策，首版支持T0的54组网格，保存全部成功、失败及取消的trial。所有指标绑定交易和净值记录，未定义指标返回null。

#### Scenario: No trades or failure
- **WHEN** 一组参数无信号，另一组运行失败
- **THEN** 前者记录合法现金路径和无交易原因，后者为failed而不是零收益

### Requirement: Time isolated selection
系统 SHALL 仅在开发区间选择参数，冻结候选后才评估未见区间，记录跨实验累计试验次数；不把已看过的历史重新命名为未见样本。证据不足只能标exploratory或insufficient_evidence。

#### Scenario: Short history
- **WHEN** 历史不足以支持预注册验证窗口
- **THEN** 可完成探索实验但不标validated，不缩短门槛来强行晋升

### Requirement: Explainable comparison
系统 SHALL 在同一时间、股票池和成本下比较固定基线、当前版及候选，按有效性、风险、验证超额、回撤、换手和稳定tie-break排序，披露绝对收益、成本、覆盖及情绪增量。

#### Scenario: No improvement
- **WHEN** 最佳候选未超过当前版或现金基线
- **THEN** 返回no_improvement，保留原版，不强制换参

### Requirement: Resumable jobs
系统 SHALL 提供任务进度、心跳、取消和重启恢复，以预算约束计算；完整结束前不得发布完整赢家。数据更新未通过质量检查时不得开始新实验。

#### Scenario: Interrupted grid
- **WHEN** 54组实验完成20组后中断
- **THEN** 恢复时保留已完成组并继续未完成组，不重复记账或把20组当完整搜索

### Requirement: Isolated challenger lifecycle
系统 SHALL 输出不可变候选版本；影子模拟只能写独立模拟子账本且不能产生可执行生产订单。默认不自动晋升，不热改运行实例或迁移持仓；风险暂停不能由调优器解除。

#### Scenario: Candidate selected
- **WHEN** 实验选出候选
- **THEN** 候选与旧策略的结果分开展示，旧运行版本和历史净值不变
