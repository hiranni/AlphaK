# Spec Delta

## Purpose

为 AlphaK 的行情、用户配置、运行状态、研究输入和模拟交易资产建立明确的数据生命周期，使缓存能安全回收、长期证据可追溯，并在重启、备份与恢复过程中保持数据边界和真实性。

## ADDED Requirements

### Requirement: Dataset lifecycle registry
系统 MUST 为每个表及混合缓存dataset登记长期、短期、运行状态或会话分类，并提供保留期、规模限制、引用保护、时间语义和政策版本。默认政策 MUST 遵循本变更 design.md 的入库数据表；新鲜度TTL MUST NOT 被当作删除期限，未登记数据 MUST NOT 自动删除。

#### Scenario: List retention policies
- **WHEN** 用户查询数据保留清单
- **THEN** 返回各数据集的现有形态、目标政策、最后成功/访问时间及受保护原因，不将未实施能力显示为已上线

#### Scenario: Unknown cache dataset
- **WHEN** 清理遇到未登记dataset
- **THEN** 跳过并报告unknown，不执行通配删除

### Requirement: Durable business data protection
系统 MUST 保留用户自选/备注/规则、历史行情财报版本、修订审计及已引用研究输入。缓存清理 MUST NOT 删除策略版本、最终决策、模拟订单/成交/持仓/净值/资金审计。策略仍只使用固定1–8只股票，不允许全市场扫描或真实订单。

#### Scenario: Cleanup with ledger fixtures
- **WHEN** 存在过期缓存及策略、订单、成交、净值、用户配置fixture并执行清理
- **THEN** 仅合资格短期数据被删，全部长期资产及其校验和保持不变

### Requirement: Research input freezing
系统 MUST 在研究任务或决策引用可覆盖数据前保存不可变输入、内容校验和、来源、可用时间及版本引用；活跃引用 MUST 阻止清理。无法获取固定输入时 MUST 返回INPUT_UNAVAILABLE，不得用当前值补造历史。

#### Scenario: Referenced snapshot survives expiration
- **WHEN** 原始报价或新闻缓存到期，但已有实验引用其冻结内容
- **THEN** 显示缓存可清理，冻结内容仍可校验读取，重跑不读取最新行情替代

#### Scenario: Concurrent freeze and cleanup
- **WHEN** 冻结输入与清理并发竞争同一内容
- **THEN** 形成有效受保护引用或明确失败，不留下指向已删除内容的成功引用

### Requirement: Observed market history and time semantics
系统 MUST 区分采集时间、上游观测时间、交易日和事前可用时间。研究使用的市场宽度 MUST 从实际观测开始积累版本化历史，区分盘中与收盘；未知时间标未知，休市日期异常行情不得成为有效最新交易日。

#### Scenario: Weekend provider payload
- **WHEN** 周日获取一份只有采集时间而无可靠交易日的新报价
- **THEN** 系统不将周日建立为有效交易日，保留异常标识，并展示最后有效样本日期

#### Scenario: Market breadth correction
- **WHEN** 同日宽度统计修订且旧版已被实验引用
- **THEN** 保存新版本及可用时间，旧实验仍解析到原版本，不伪造之前未采集的日期

### Requirement: Previewable bounded cleanup
系统 MUST 提供清理预览、计划执行与执行记录；默认dry-run，显式执行仅接受10分钟内有效plan_id。执行 MUST 重新校验政策、引用、pin和活动租约，分批可重入，记录实际删除数量与保护项；清理失败不得伪报成功。

#### Scenario: Plan becomes stale
- **WHEN** 预览后数据被pin或政策版本改变
- **THEN** 执行跳过新受保护项或拒绝旧计划并报告原因，不依据旧估计强删

#### Scenario: Interrupted cleanup
- **WHEN** 执行进程在批次之间停止后重启
- **THEN** 已提交结果可查，未完成部分可安全重试，不重复删除保护数据

### Requirement: Explainability retention boundary
系统 MUST 遵循既有可解释契约，默认季度粒度保留8季度，pin豁免；过期详细包清理 MUST 留period/hash/size/deleted_at墓碑及审计，不删除长期账本与决策摘要。已过期回放 MUST 返回CACHE_EXPIRED或REPLAY_UNAVAILABLE，回放 MUST 只读。

#### Scenario: Expired decision traces
- **WHEN** 季度详细轨迹包过期并清理后用户请求回放
- **THEN** 返回过期状态及墓碑，净值/交易仍可查询，系统不拼造原始Agent响应或生成订单

### Requirement: State-aware operational retention
系统 MUST 保护运行中任务、未完成重试、同步水位与提醒冷却；终结任务、提醒事件和暂存按分类保留，不能由统一时间阈值删除活动状态。

#### Scenario: Restart during retry
- **WHEN** 同步重试尚未终结并发生进程重启与缓存清理
- **THEN** 重试次数、水位和冷却状态仍可恢复，不造成重复提醒或重复导入

### Requirement: Consistent backup and restore
系统 MUST 使用一致性在线备份并校验完整性，按7天日备份、4份周备份及至少30天迁移前备份的默认政策轮换，保护手动/pin和最后有效备份。恢复 MUST 先备份当前状态，校验hash/schema，暂停写入；恢复后清理必须重新预览。密钥 MUST NOT 出现在普通备份manifest。

#### Scenario: Backup rotation failure
- **WHEN** 新备份校验失败或无其他有效备份
- **THEN** 保留旧有效备份并报告失败，不为满足容量目标删除最后可恢复副本

#### Scenario: Restore old retention settings
- **WHEN** 恢复包含旧保留政策的有效备份
- **THEN** 在维护状态验证关键数据，重新规划清理后才恢复清理调度，不立即按旧TTL删除数据

### Requirement: Persistence documentation harness
系统 MUST 在构建、测试与CI校验实际迁移后的表、字段、索引、约束及保留政策与 design.md 一致。新增、修改或删除持久化项 MUST 同步更新设计的生命周期说明和实际结构表，未知项必须使检查失败。

#### Scenario: Schema changes without documentation
- **WHEN** 新增字段或索引但没有同步更新设计
- **THEN** harness失败并指明更新设计，不允许构建通过
