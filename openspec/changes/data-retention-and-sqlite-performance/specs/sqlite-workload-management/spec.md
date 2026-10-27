# Spec Delta

## Purpose

管理 AlphaK 秒级行情、历史读取和维护任务共享 SQLite 的负载，通过有界批量更新、明确的新鲜度与错误状态、有限缓存和可验证的延迟指标，保障本地工作台持续可用而不损害研究证据或资金账本。

## ADDED Requirements

### Requirement: Atomic realtime batches
系统 MUST 将每批最多20只报价作为单个原子提交单元，整批先校验，失败整批回滚且返回错误。重复相同观测 MUST 去重，保留真实数据时间与独立检查时间，不能因轮询将旧行情标为新成交。

#### Scenario: Invalid member in batch
- **WHEN** 20只报价中有一只验证失败
- **THEN** 整批不落部分新状态并返回明确错误，其他已成功批次不回滚

#### Scenario: Repeated unchanged quote
- **WHEN** 两次请求返回相同上游观测
- **THEN** 最新报价内容不重复写入，检查时间可变化但行情时间不变化

### Requirement: Bounded contention and database errors
数据库操作 MUST 有有限锁等待与超时反馈，网络和研究计算 MUST 不占数据库临界区。数据库繁忙或写失败 MUST 暴露可重试错误，读端保留最后成功值并标明新鲜度，不清空页面或无限阻塞。

#### Scenario: Slow upstream
- **WHEN** 上游请求阻塞30秒
- **THEN** 已缓存行情查询仍可完成，数据库写锁不等待网络返回

#### Scenario: Write timeout
- **WHEN** 锁等待超过配置预算
- **THEN** 返回DB_BUSY等分类错误，保留上次成功数据，记录耗时与有限重试

### Requirement: Bounded incremental chart storage
秒级图缓存 MUST 按标的、周期、时间桶和来源去重更新，仅保存新柱或变化柱；默认每键最多360根，空闲7天回收，活动订阅/刷新中保护。默认 MUST NOT 保存全量3秒快照历史，研究引用窗口另行冻结。

#### Scenario: Repeated open bucket
- **WHEN** 相同时间桶被连续轮询且上游修订当前柱
- **THEN** 更新该桶的OHLCV而不追加重复柱或重复累加volume，历史研究输入不被覆盖

#### Scenario: Active long session
- **WHEN** 页面连续使用数小时
- **THEN** 活动键仍受360根上限约束，不因持续访问无限增长

### Requirement: Subscription-aware sampling
系统 MUST 按活动订阅、上海交易日和采样窗口执行高频请求，单次至多20只、批次不重叠。无订阅、休市或故障时 MUST 停止不必要采样并展示最后成功数据/真实时间；慢轮询不得积压任务。

#### Scenario: No active clients
- **WHEN** 最后一个客户端离开且30秒订阅租约到期
- **THEN** 该订阅的高频网络采样停止；日线补齐任务不受影响

#### Scenario: Slow batch cycle
- **WHEN** 全部自选一轮请求耗时超过3秒
- **THEN** 下一轮等待当前轮结束，不并发积压，并如实显示更新时间

### Requirement: WAL observability and durable maintenance
系统 MUST 验证既有WAL、外键与每连接锁等待配置，保留耐久写入；MUST 暴露DB/WAL大小、提交/锁等待耗时及checkpoint结果。维护不得在正常请求中强制全库压缩或删除活动WAL；磁盘满必须报告失败而非自动删除长期资产。

#### Scenario: Long reader blocks checkpoint
- **WHEN** 活动读事务使WAL检查点无法完成
- **THEN** 记录阻塞并低峰重试，不强制删除WAL，不中断已提交数据读取

### Requirement: Reproducible workload acceptance
系统 MUST 在固定硬件与假上游下记录37/120只每3秒、市场查询及秒图混合负载的基线和优化结果。正常负载目标为本地读API p95≤200ms、提交p95≤50ms、锁等待p95≤50ms、DB_BUSY=0；未达标 MUST 保留失败记录并完成连接隔离复测，不将目标描述为已测成绩。

#### Scenario: Acceptance run
- **WHEN** 按design D7执行2分钟预热和15分钟采样
- **THEN** 报告含延迟分位数、写入字节、文件增长、事务数和资源配置；网络延迟单列，每批提交不超过一次
