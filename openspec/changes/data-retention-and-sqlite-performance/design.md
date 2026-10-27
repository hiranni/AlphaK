# AlphaK 数据持久化与 SQLite 优化设计

## Context

本设计直接列出全部入库数据及其保留政策，作为实施依据。已核查 `db.rs`、`sources/store.rs`、`routes.rs::realtime_quotes`、`live.rs`、`market_breadth.rs`、`workbench.rs`、`scripts/backup.sh` 和既有策略契约。当前是单 SQLite Connection + Arc<Mutex<_>>，WAL及foreign_keys已启用；批量报价逐行INSERT OR REPLACE，锁内构建响应；秒线JSON整段覆盖。没有显式busy_timeout配置，具体默认依驱动不可当作SLA。备份脚本已有在线.backup及SHA256，但无统一轮换。

已发布spec为空；在途 alphak-platform-mvp 的 platform-foundation/market-data 涵盖配置、备份恢复与历史采集。本提案补充生命周期和高频负载要求，不替换其契约，也不扩大数据采集范围。

## Goals / Non-Goals

**Goals:** 明确每个数据集的所有者、时间语义和删除资格；保障研究输入/账本；量化并降低写放大和锁阻塞。

**Non-Goals:** 不实现量化/回测/自动调优引擎，不承诺有完整历史情绪和PIT数据；不切换数据库，不默认订阅全量秒级历史，不删除既有异常日期数据。

## 入库数据、保存期限与清理规则

核查日期：2026-10-25。本文中的“当前”来自代码；“目标”是待实施政策，不代表 TTL、归档或自动清理已经上线。长期表示不因缓存清理自动删除，不表示永远不可由用户显式删除。短期持久化也落磁盘，可跨进程重启，但有保留上限。新表名是设计名称，迁移前须核对。

### 1. 分类与统一口径

- L：长期业务/历史数据，默认无自动 TTL。更正采用版本或修订审计，已被实验引用的内容不得覆盖。
- S：短期持久化，可重建显示缓存、事件历史、任务诊断；按以下政策清理。
- O：可覆盖运行状态，重启需恢复；生命周期按状态，不按统一天数删除。
- E：浏览器/内存临时状态，不作为研究或交易依据。
- P：未来能力，当前没有业务服务/表。必须在接入前登记；本变更不实现策略引擎。

所有天数均为自然日（UTC 时长）；“交易日”明确按 Asia/Shanghai 日历。新鲜度 TTL 只决定是否刷新，不等于落盘保留期。fetched_at、provider_time、trade_date、available_at 分别表示采集、行情、归属交易日和事前可用时间，不可相互冒充；未知上游时间必须标 unknown。

### 2. 已有关系表逐项盘点（29 张）

现有表的逐项内容与目标政策如下。目录、当前配置及运行水位按业务键保存当前值；日线和日估值按标的/日期 upsert；版本财报和审计保留版本/事件；用户自选由明确业务操作增删改。当前写入方式不等于目标已具备不可变历史，以下保护和期限是本提案待实施要求。

| 已有表名 | 入库内容与当前写入方式 | 持久化类别 / 目标保留期限 | 删除条件、保护与研究约束 |
|---|---|---|---|
| instruments | 证券资料，按 symbol 保存当前值 | L，持续保留 | 删除样本资格不删除研究引用证券；退市不抹历史；需另行补版本才可称历史成分 |
| trading_calendar | 每日开休市 | L，持续保留及修订版本 | 日期治理基础；周末/明确休市的异常行情隔离 |
| quote_daily | 个股 OHLCV、复权信息，symbol/date 主键 | L，持续保留 | 当前同键更新不是不可变历史；新增修订指纹，实验冻结输入后不依赖活表 |
| financial_report | 财报版本、announce/effective 时间和 hash | L，所有版本 | 保存公开与采集时间；更正不得覆盖已引用版本 |
| financial_indicator | 财务衍生指标，symbol/report_period | L | 当前未独立版本化，研究使用须绑定财报与算法版本 |
| daily_basic | 每日估值、股本、市值 | L | 缺失不填零；更正需可追溯 |
| company_profile | 当前公司资料 | L，保留当前值 | 涉及研究的字段冻结到输入包；当前表不代表完整历史 |
| index_catalog | 指数目录 | L | 同证券目录 |
| index_quote | 指数日线 | L | 交易日、来源、修订校验；不接受周日生成新交易日 |
| index_member | 带 in/out 日期的指数成分 | L | 已有字段不等于供应商提供完整 PIT 历史 |
| sector_catalog | 板块目录 | L | 稳定 ID 与修订记录 |
| sector_member | 带 in/out 日期的板块成员 | L | 冻结历史成员，不用当前成员倒推 |
| watchlist_group | 分组与顺序、颜色 | L，用户管理 | 不受行情缓存清理影响；保护“全部自选”系统身份 |
| watchlist_item | 关注关系、分组、备注、顺序 | L，用户管理 | 取消关注是明确业务操作，不触发历史行情级联删除 |
| app_setting | 应用偏好 | L | 可导出备份，不保存原文密钥 |
| data_source_config | 数据源启用与环境变量引用 | L | 保留 secret_env 引用，不导出环境密钥 |
| backfill_audit | 历史回填审计 | L | 不由任务日志轮换删除 |
| reference_correction | 参考数据修订审计 | L | 研究追溯保护 |
| quote_snapshot | 每标的一条最新快照，覆盖更新 | S：保留最后成功值；取消跟踪且30天未访问可回收 | 没有每3秒完整历史；交易决策引用前先复制为长期输入 |
| market_snapshot | 每日期一份概览 JSON | S：90天 | 可重算显示数据；日期不同的样本不可混作同日；研究宽度另存 |
| live_cache | 秒/分钟图、基本面补充、板块、热度、新闻、宽度等 JSON 混合 | S：按 dataset 拆分，见下表 | 目前覆盖值而非历史；TTL 不等于已自动删除 |
| quote_staging | 导入校验暂存，valid 发布后会删除 | S：终结批次7天；失败/隔离30天 | 未完成/重试引用不删；只保存脱敏诊断摘要到长期审计 |
| data_job | 同步任务、进度、幂等键 | O + S：运行中保护；成功30天、失败90天 | 活跃/租约未过期不清；幂等键另留90天，重放安全不得依赖已删日志 |
| source_retry_task | 待重试来源范围、计数 | O：未终结保留 | 终结转任务记录，保留90天；重启保留重试状态 |
| sync_checkpoint | 增量同步水位 | O：当前值长期保留 | 清缓存不能退水位或重导全部历史 |
| data_freshness | 各数据集最后成功/失败状态 | O：覆盖当前值 | 重启仍须显示 stale/错误，不伪造新鲜度 |
| quality_report | 当前覆盖率与异常统计 | O：覆盖当前值 | 被实验引用的报告复制冻结，不以当前报告覆盖历史证据 |
| watch_alert | 用户提醒、启停、冷却和最后观察 | L + O：规则用户管理；状态持续保留 | 重启不能重置冷却造成重复提醒 |
| watch_alert_event | 提醒触发历史 | S：180天 | 策略信号与订单证据不可仅放此表；研究引用的事件先冻结 |

### 3. live_cache 拆分政策

| 数据族 | 当前路径/用途 | 目标磁盘保留/规模上限 | 长期研究证据 |
|---|---|---|---|
| second_* | AxData秒线；当前360根JSON覆盖，3s TTL为3秒 | 迁为逐bar键；每symbol/周期最近360根且7天未访问回收 | 默认不采全量；被引用窗口复制到冻结输入包 |
| minute_* | 分钟走势图缓存 | 保留接口当前有效窗口，空闲7天回收；每键最多1024根 | 有研究需求再冻结区间，不能从滚动缓存推断完整历史 |
| snapshot | 当前报价补充 | 与 quote_snapshot 去重统一，保留上次成功值；失去跟踪且30天未访问清理 | 决策输入另存 |
| metrics/profile/financials | HTTP补充原始JSON | 最后成功值，空闲30天回收 | 验证后才能发布到日估值/财报；有缓存不等于规范历史表已入库 |
| sectors/popularity | 板块热度、热门榜 | 空闲7天回收，仅保留当前成功payload | 如参与情绪模型则按观测追加证据，不能覆盖 |
| news | 新闻列表，外部不可信文本 | 空闲7天回收、按条去重；当前列表不是新闻全文档案 | 引用时保存正文/摘要、来源、发布时间、首次可见时间、hash及授权限制；密钥脱敏 |
| MARKET/breadth | 乐咕乐股大盘宽度，当前只保存最后成功payload | 显示缓存保留最后成功值，30天未访问回收 | 新增每日版本化宽度历史（L）；只从真实观测开始积累，禁止伪造过去历史 |
| watch_history | 自选日线补齐任务状态，按标的覆盖 | O，未完成与重试状态持续保护，不自动TTL清理 | 不等于行情历史；完成时间不得替代真实交易日 |
| 未知dataset | 新模块或遗留数据 | 未登记前禁止自动删除，报告unknown | 新数据写入路径须注册所有者和保留政策 |

热键的360/1024根上限即使持续访问也执行；空闲TTL按 last_accessed_at，绝不能用 fetched_at 代替访问时间。缓存清理时保护当前订阅和正在刷新键，终止订阅后按空闲期回收。

### 4. 文件、浏览器与内存（不属于业务入库表）

| 资产 | 类别/目标 | 注意事项 |
|---|---|---|
| SQLite db / -wal / -shm | 数据库整体 | WAL不是历史归档；运行中不可手工删WAL或仅复制db充当一致备份 |
| .env/环境变量 | L，人工管理 | 数据源凭据独立安全保管；DB备份不自动包含这些密钥 |
| backups/*.db + .sha256 | S，可 pin | 自动普通备份7天日备份+4个周备份；迁移/恢复前快照至少30天；至少1份成功验证；手动/pin不自动删 |
| 应用logs | S，14天且累计256MiB，按先触发上限轮换 | 脱敏；不替代长期决策/资金审计 |
| 导入文件与拒绝行 | S，成功7天/失败30天 | 研究引用先入manifest；不默认扫描或清理项目外用户原件 |
| CSV用户导出 | 用户管理 | 自动清理不删除用户指定导出目录 |
| docs/research与验证报告 | L，Git版本管理 | 文献与数学演示不是平台策略实证结果 |
| dist/target/node_modules | 可重建构建产物 | 不纳入数据备份；clean不得删除数据库 |
| localStorage：focus/fields/view、浏览器提醒开关/水位 | 浏览器本地持续偏好 | 不是服务器数据库资产，换浏览器不自动恢复；清行情缓存不清这些项 |
| React实时报价与spark采样、筛选/滚动 | E，页面会话 | 不能当永久行情历史或回测输入；刷新页面可能重置 |

### 5. 未来策略资产（尚未实现）

| 资产 | 目标 |
|---|---|
| 策略定义/不可变版本、实例、Agent profile版本、风险与晋升政策 | L；实例绑定Agent，策略定义不声明Agent；每策略最多8只 |
| 输入manifest、被引用原始数据包、因子原始值/版本、Q/O快照、情绪观测、日历/公司行动/复权版本 | L；内容寻址、校验和、引用计数，保证重现；不能只保存文件路径 |
| experiments/trials、参数、随机种子、时间切分、候选/冠军、失败原因 | L；包括失败及no_improvement，不只保留赢家 |
| 回测净值/基准/指标/交易、样本外结果 | L；绑定不可变版本和输入，历史结果不得被新参数覆盖 |
| 结构化信号、Agent最终决策、校验与风控结果 | L；至少保留足以解释最终动作的结构化依据 |
| 模拟资金、分配、订单、成交、持仓变更、每日净值、对账与资金审计 | L；绝不由任何缓存TTL清理；Agent不能直接写订单表 |
| 可解释详细轨迹包：prompt/response/tool traces等 | S，依既有schema：默认季度、8季度；pin豁免，删后留墓碑 | 

可解释缓存到期后，净值/交易/决策摘要仍可查；详细回放返回 CACHE_EXPIRED/REPLAY_UNAVAILABLE。长期输入用于确定性重算，不承诺能重现已删除的LLM原始响应。若要支持原始响应回放，应在到期前pin整个包，而不是事后拼造。

### 6. 新增入库结构与写入方式（实施状态见下表）

生命周期基础设施表已提供幂等迁移；策略研究与交易结构仍属于后续能力接入要求，不代表引擎已实现。表存在不等于每项清理政策均已启用，实际启用范围见实施状态。所有长期表默认无自动 TTL。

| 范围 / 拟新增表或数据集 | 入库数据 | 写入方式 / 唯一性与关联 | 类别 / 保留期限 | 清理与保护 |
|---|---|---|---|---|
| 本提案：lifecycle_policy | 数据族、所有者、政策版本、期限、行数上限、用户覆盖 | 按数据族保存生效版本及变更记录 | L，政策版本长期 | 被清理记录引用的政策版本不可丢失 |
| 本提案：input_manifest / data_reference | 输入包 hash、位置、大小、来源、可用时间、引用主体 | 文件内容寻址；manifest 与引用事务提交 | L，已引用输入长期 | 引用或 pin 存在禁止回收；不只保留路径 |
| 本提案：bar_cache | 秒/分钟 OHLCV、桶时间、来源、修订、hash | symbol / period / bucket_start / source 唯一；只更新变化柱 | S，秒线每键360根、分钟每键1024根；空闲7天 | 活动租约保护；研究引用先冻结，不能把缓存当完整历史 |
| 本提案：market_breadth_observation | 大盘涨跌分布、范围、交易日、真实观测与可用时间 | 同日按来源和修订版本追加，日级研究历史 | L，长期 | 不由显示缓存 TTL 删除，不用最新值补造过去 |
| 本提案：subscription_lease | 订阅键、客户端标识、最近心跳、到期时间 | 心跳覆盖，30秒租约 | O，过期可回收 | 活动租约阻止对应缓存清理，退出停止高频采样 |
| 本提案：cleanup_plan | 候选范围、政策版本、估算量、保护原因、执行状态 | 预览生成不可变计划，执行重验 | S，执行资格10分钟；终态诊断90天 | 过期计划禁止执行，审计另存不随计划清除 |
| 本提案：cleanup_audit | 计划、实际删除数、跳过原因、操作者与时间 | 每次执行追加 | L，长期 | 不含敏感 payload，清理不能删除自身审计 |
| 本提案：cache_tombstone | 包标识、季度、hash、大小、删除时间与原因 | 每个被删除解释包追加墓碑 | L，长期 | 用于明确返回 CACHE_EXPIRED，不级联账本 |
| 本提案：job_idempotency | 请求幂等键、任务身份、最终结果摘要 | 幂等键唯一，原任务清理后仍能识别重放 | S，任务终结后90天 | 活跃任务不删；到期后的重复请求语义须显式定义 |
| 后续：strategy_version / strategy_instance / agent_profile_version | 策略规则、参数、实例绑定、Agent与风控政策版本 | 版本不可变；实例引用固定版本 | L，长期 | Agent只绑定实例；每策略最多8只；不以新版本覆盖历史 |
| 后续：factor_value / score_snapshot / sentiment_observation | 因子原始值、标准化、贡献、质量/机会分、情绪观测 | 按标的、as_of、模型/因子版本和输入包追加 | L，长期 | 保留缺失、来源和 available_at；引用不得覆盖 |
| 后续：research_reference_snapshot | 研究使用的日历、公司行动、复权、行业成员版本 | 内容 hash 与输入 manifest 关联 | L，长期 | 不用当前参考数据替代过去版本 |
| 后续：experiment / optimization_trial | 参数、种子、时间切分、成本、候选/冠军、失败原因 | 每次实验和试验追加，绑定输入与代码版本 | L，长期 | 失败及无改进结果同样保留，不只存赢家 |
| 后续：backtest_run / backtest_nav / backtest_trade / validation_result | 净值、基准、交易、统计、样本外和稳健性结果 | run_id 关联固定版本，明细追加 | L，长期 | 新参数新运行；清缓存不删验证证据 |
| 后续：strategy_signal / agent_decision / decision_validation | 触发和失效条件、最终决策、schema/业务/风控校验 | 事件追加；决策绑定信号、输入和策略实例 | L，长期 | 最终结构化依据保留，Agent不得直接写订单 |
| 后续：paper_account / allocation / orders / fills / position_change / nav / reconciliation | 虚拟资金、仓位分配、订单成交、持仓变动、净值和对账 | 账本事件追加；当前持仓由账本投影，关联决策 | L，长期，无自动TTL | 任何缓存清理不得触碰；不连接真实订单 |
| 后续：explainability_cache | prompt、response、工具轨迹与详细分析包 | 按既有契约季度分包，manifest 关联内容包 | S，默认8季度；pin豁免 | 到期删详细包并留墓碑；最终决策、交易、净值不删；回放只读 |

### 7. 负载与容量假设

37只/3秒约12.3行更新每秒；120只约40行更新每秒。37只在4小时交易时段追加快照约177,600行/日，按250日约44,400,000行/年，仅估行数，不估字节。现有最新值覆盖不是这个增长模式。秒线JSON仍有反复写放大；SQLite并发能力需实测，不以理论行数保证延迟。数据采集仅覆盖已跟踪/导入样本，不扩大全市场。

## Decisions

### D1. 保留注册表与引用保护

以服务端版本化注册表声明每个table/dataset的class、policy_version、retention、max_rows、cleanup_key、owner、references。数据库保存用户覆盖和上次运行状态。默认值见本设计“入库数据、保存期限与清理规则”各表，未知数据只报告不清理。长期业务显式删除与缓存清理分开。

研究输入采用内容寻址文件包+SQLite manifest/ref表，写临时文件、校验后原子rename，再提交DB引用；crash遗留包进入隔离回收队列，禁止随意删活动引用。没有文件时任务失败为INPUT_UNAVAILABLE；不会从当前行情补造旧输入。清理仅能选白名单；引用检查与删除在同一写事务，freeze/pin也使用该写边界。过期可解释包独立状态机，不给交易表任何级联删除。

替代：只给所有表统一TTL会破坏研究与账本，因此拒绝；全量原始响应永久保留会无限增长且可能含敏感信息，默认拒绝。

### D2. 显示缓存与情绪历史分离

新增市场宽度观测的版本化日级历史，保存真实observed_at、provider_time（未知可null）、trade_date、scope、source、数值、hash、available_at。盘中观察不能冒充收盘值；同日修订保留版本，研究按当时可用版本选择。新闻/热度只有被模型使用才冻结输入，遵守来源授权。既有latest JSON不倒填历史。

时间校验基于上海交易日历，采集时间不赋给交易日。异常记录隔离/标记，不自动更正成某个过去日期；样本与指数可有各自日期并显示，不把缺数据算平盘。此变更补日期元数据和校验，不重新抓取全市场数据。

### D3. 批量事务与连接策略

先解析、验证整批（最多20只）、准备响应，再短暂获取写锁；一批一个事务upsert并提交。任一非法记录整批回滚并返回明确错误；之前成功批次不回滚。相同symbol+相同观测hash去重，不改变provider_time；另存last_checked_at用于显示轮询状态。不能以未变化行情重写成交时间。

网络、JSON解析、因子与回测计算不在数据库锁/事务内。同步SQLite操作放有界blocking执行器，busy_timeout每连接设置1000ms，应用锁获取预算1000ms；超时分类返回DB_BUSY及可重试状态，UI保留上次值。后台重试最多3次、退避并计预算，不无限持锁等网络。

先保持单写连接，不立即上复杂连接池。若120只混合负载超过D7门槛，下一任务引入最多2个独立只读连接，逐一设置busy_timeout，读事务读完即释放。WAL无法解决应用共享Mutex串行问题；不把启用WAL等同于已读写并行。

### D4. WAL、持久性与维护

保留WAL，启动读取并报告实际journal_mode、foreign_keys、busy_timeout、synchronous、wal_autocheckpoint；写连接目标synchronous=FULL，自动checkpoint起点1000页。在持久DB验证，内存测试不强行要求WAL。高频写批次合并优先于降低耐久性。

监控db/wal/shm大小、page/freelist、写事务数、行数、字节、锁等待、busy次数、SQL耗时、checkpoint结果和最老读事务。WAL超过64MiB报警并尝试低峰PASSIVE checkpoint；有读者阻碍时记录重试，不强制TRUNCATE。无活动读者且低峰时才允许受控TRUNCATE；不得每轮报价执行。VACUUM仅离线/维护显式执行，检查磁盘余量，不在HTTP请求或每次清理后执行。

### D5. 高频数据增量与订阅

新增短期bar表，键为(symbol,period,bucket_start,source)，保存OHLCV、source_revision、observed_at和hash；当前未闭合柱允许更新，历史更正附修订标识。每轮upsert新/变bar；去重重复HTTP结果，累计volume/amount不跨快照直接相加。3秒bar由1秒bar重新聚合对应桶而非反复叠加，保留当前未闭合标记。每键360根上限和空闲7天双约束，删除小批完成。

当前AxData可能返回整个秒线历史：本地去重只能减少落盘，不能声称减少网络payload；上游增量参数须能力探测后启用。HTTP响应继续返回既有BarsResponse，存储迁移不强迫前端变更。默认仅活动图请求秒线；自选报价仍每批20只，串行、无重叠，慢轮不堆积。

新增活动订阅心跳租约（30秒），最后订阅退出后停止高频网络采样；仅上海交易日和交易时间窗口采样，盘前/午休/盘后分别使用明确窗口，盘后允许一次终值补取。休市展示最后成功值与真实行情时间；不能用请求时间制造实时性。故障保留旧值并标stale。长期3秒追加采集是独立显式开启的能力，必须有配额和保留政策，本次不默认启用。

### D6. 可预览清理与安全备份

新增管理接口（实现时写入OpenAPI）：GET /api/v1/data/retention 展示policy、大小估计和引用；POST /api/v1/data/cleanup/preview 返回plan_id、policy_version、候选行/字节估计、保护原因和有效期；POST /api/v1/data/cleanup 执行指定plan_id（dry_run默认true，false需显式指定），GET /api/v1/data/cleanup/{id} 查询结果。只读接口不触发删除。计划10分钟过期；执行时重验引用/pin/租约及策略版本，变化项跳过并报告。成功返回实际删除数和tombstone/audit，不能将估计字节当文件已缩小。

后台清理每日低峰执行同一规划器，白名单分批最多1000行/事务，遇订阅写入/锁繁忙退让；中断可重入。可解释包采用deleting→deleted和墓碑，失败可恢复，不半删伪成功。墓碑含数据族、period、hash、大小、删除时间、原因、操作者；删除payload后不保留敏感全文。

备份使用已有SQLite在线backup，不复制运行中db文件；保存schema、政策版本、校验和及脱敏配置manifest，密钥与浏览器偏好不隐式打包。按本设计文件资产表的期限轮换，必须有已通过校验的替代备份才删除旧自动备份；pin/手动不轮换。恢复先备份当前状态，校验hash+integrity/schema，所有写入停机切换，失败恢复原库；恢复后先进入维护状态，重新预览清理再恢复调度，防止旧政策立即删数据。

### D7. 可执行性能验收

在同一机器、固定记录硬件/SQLite版本/电源模式上，以假上游隔离网络，分别37和120只、每3秒一轮、单批20只，叠加市场查询每10秒、一个360根秒图每3秒、60秒历史读取（加载后锁外计算），预热2分钟采样15分钟。记录p50/p95/p99、CPU/RSS、吞吐、DB/WAL增长和写入字节；另做30秒慢上游、5秒读事务、写故障与磁盘不足场景。

验收目标：本地缓存读API p95≤200ms、事务提交p95≤50ms、锁等待p95≤50ms，正常负载DB_BUSY=0；每批至多一次commit，重复不变数据不重写bar内容，停止订阅后停止高频采样、清理后行数有界。网络超时单列，不计入本地DB目标。baseline与优化使用同数据同时间；不达标保留失败报告，触发D3读连接方案并复测，不调低FULL掩盖。目标是待验证门槛，不是当前测量成绩。

## Risks / Trade-offs

- [历史原表会被更新，现有PIT不完整] → 固定输入包，从采集起保留修订；不宣称补回未知历史。
- [输入包长期增长] → 只冻结实际引用区间、按hash共享、展示空间；删除实验不能隐式删除账本或用户pin。
- [短期bar迁移或清理竞态] → 影子验证、引用/租约事务检查、失败保留旧缓存。
- [磁盘满影响写入与备份] → 拒绝新采集并显式告警；不自动牺牲账本、研究证据或最后有效备份。
- [FULL与1000ms锁等待可能影响延迟] → 用批量提交和隔离读降低争用，指标验证，保持错误可见。

## Migration Plan

1. 只读采集实际schema、行数、文件大小及PRAGMA，记录版本，在线备份校验。
2. 新增保留政策/引用/审计/短期bar/观测历史结构，默认不启用自动删除；旧live_cache可读。
3. 先部署批量事务、锁外计算与观测，再影子双写bar，对比OHLCV/时间/重复请求；不补造缺失历史。
4. 切换bar读取，保留旧payload至少7天；只清理已验证迁移副本，回滚可暂回旧读取。
5. 性能验收后开启preview，验证保护不变量，再启用默认短期清理和备份轮换；长期数据不设TTL。
6. 回滚先停止清理/订阅任务、恢复旧读取；破坏性schema回退仅通过已验证备份且停止写入，恢复窗口后的数据损失须事先显示，不能静默覆盖。策略/账本保护用契约fixture验证，不能宣称尚未实现的服务已完成验收。

## 实施状态（持续更新）

| 数据 / 能力 | 当前已落地行为 | 尚未启用的政策 / 边界 |
|---|---|---|
| 实时报价 | 每批最多20只，先完整校验；单事务，hash相同不更新报价内容；独立last_checked_at | 上游接口没有可信时间，provider_time为null，界面不得将检查时间称行情时间；不保存全量3秒快照历史 |
| 高频图 | bar_cache增量覆盖桶，内容hash去重，修订号；秒线360、分钟1024根 | 旧JSON保留作为回滚副本；上游仍可能返回全历史 |
| 短期清理 | 仅白名单live_cache数据族和bar_cache可通过10分钟计划显式执行；默认dry-run；政策文件指纹变化拒绝旧计划；恢复维护模式禁止执行；事务内重验租约和空闲时间；每事务最多1000行并审计 | 自动清理默认关闭；任务/提醒/报价等期限尚不自动执行，保留原数据 |
| 输入证据 | 内容寻址输入包、manifest及引用；校验读取；显示缓存清理不涉及冻结包 | 策略引擎接入、孤儿包回收及解释包到期状态机另行验收 |
| lifecycle_policy | 数据族与政策版本登记 | 当前发布默认政策，不开放任意用户覆盖期限 |
| data_reference | owner与input_hash关联并保护冻结研究输入 | 不直接指向滚动缓存行 |
| subscription_lease | 已建30秒租约结构，供活动订阅接入 | 自选/个股页面心跳续租，隐藏时停止轮询；上海交易窗口外只读最后数据，不持续3秒请求 |
| cleanup_plan | 预览候选、版本、10分钟资格和执行状态 | 终态诊断90天轮换尚未启用 |
| cleanup_audit | 每批实际删除行数、键、执行时间长期追加 | 不自动删除审计 |
| cache_tombstone | 已建详细解释包墓碑结构 | 解释包清理状态机尚未接入，禁止用此表存在宣称回放可用 |
| job_idempotency | 已建独立幂等记录结构 | 任务日志清理前仍须完成接入；当前不清任务 |
| SQLite | WAL保留，busy_timeout=1000ms、FULL、autocheckpoint=1000；新增路径有界blocking和1秒锁预算；容量诊断 | 旧调用路径仍需逐步迁移；性能目标须经D7实测 |
| 备份恢复 | 在线backup后复制manifest引用的不可变输入包；全部校验后发布manifest；恢复拒绝活动DB，先备份当前状态，恢复后维护模式、自动清理关闭 | 自动备份7天日备份+4周；手动/pin和迁移前备份不自动删除，比30天最低要求更保守；验证失败整轮不删旧备份 |

## 持久化结构契约与 harness

新增、删除或修改表、字段、索引、约束及保留政策时，必须同步更新本设计的生命周期表和下面的实际结构表。`make harness` 与 CI 校验真实迁移结果及政策指纹；不允许仅改代码。结构表由 `make persistence-doc-update` 更新，人工仍须核对用途、保存期限与删除保护。探测使用内存库，不访问用户数据库。

### 策略研究观察持久化（2026-10-25 增补）

| 表名 | 入库内容与写入方式 | 类别 / 保留期限 | 删除与研究约束 |
|---|---|---|---|
| quant_monitor | id 主键；name、family、definition（冻结 V1 规则、参数、最多8只股票）、enabled、created_at；仅 enabled 可经 API 更新；补齐策略 INSERT OR IGNORE | L，长期，不自动删除 | 不含 Agent、账户与订单；研究观察定义不是可交易策略契约；修改定义需创建新 ID/版本，不能覆盖历史 |
| quant_observation | id 内容键；monitor_id 外键；symbol、as_of、input_hash、payload、created_at；唯一约束 monitor_id/symbol/as_of/input_hash；created_at 倒序索引 | L，长期，不自动删除 | payload 冻结规则、计算版本、历史 OHLCV 输入来源/采集时间、情绪股票池、指标与解释；相同输入去重。当前历史重建不冒充 PIT；不是净值、收益、成交或账本；回放只读。后续扩展字段必须更新本表与 schema harness |

两表在 db::init_db 建立，纳入 WAL、备份与生命周期注册。观察以每策略每标的每数据版本入库，不以每分钟全量重复存盘；策略上限8只，13套内建策略，没有全市场扫描。当前不清理研究输入；压缩/归档需要后续显式保留设计。周末和已知休市日数据不参与；未知日历使用工作日研究过滤并明确限制。原始日线可修订，但已冻结观察输入不随源表更新。

### 实盘研究配置与分析（2026-10-26）

| 表名 | 入库字段 / 约束 | 生命周期 | 删除与业务边界 |
| --- | --- | --- | --- |
| research_workspace | id TEXT 主键、config TEXT、created_at TEXT，均非空；config 包含名称、1–8只不重复沪深股票、唯一模型ID与权重、情绪范围；API严格字段与业务校验；配置不可变，修改保存新ID | L，长期，不自动删除 | 手动研究配置，不是 strategy-instance 账户/Agent执行契约；不创建账本，不对监控任务自动启用 |
| research_analysis | id TEXT 主键、workspace_id TEXT 外键、input_hash TEXT、input TEXT、report TEXT、created_at TEXT，均非空；UNIQUE(workspace_id,input_hash)；索引(workspace_id,created_at DESC) | L，长期，不自动删除 | 冻结配置、模型说明、执行代码指纹、OHLCV来源/采集时间、截止日、报告、18组候选评分、近邻标签日期；报告不是成交或净值。相同语义输入/截止日去重，采集时间单独变化不重复计算；清理不得删除输入，回放只读 |

两表与其他长期研究资料统一纳入迁移、WAL与备份注册，无TTL清理入口。单次读取最多8×400日线，计算在锁外且并发上限1；列表最近100配置。不存在真实订单或Agent直接写表。量化中心仅定义，股票与资金权重保存在实盘研究配置。历史行情和日历能力不足必须阻断或明确限制。

### 实盘模拟资金初始化（2026-10-26）

| 入库表 | 字段及约束 | 用途 | 生命周期 |
| --- | --- | --- | --- |
| paper_strategy | id 主键、request_key 唯一、workspace_id 唯一外键、initial_cash_fen 正整数（分）、policy JSON、created_at | 冻结股票池/模型配置引用、初始资金和 A 股执行政策；仅登记，无自动交易 | 长期，禁止自动清理 |
| paper_cash_ledger | id 主键、strategy_id 唯一外键、amount_fen 正整数、kind 仅 initial_funding、created_at | 独立初始入金流水；同一策略只入金一次 | 长期账本，禁止 TTL 清理 |

策略、研究配置和入金在一个事务内创建；重复请求必须匹配原配置，禁止重复注资。金额为人民币分。当前尚无撮合、持仓或净值引擎，状态固定为待接入；A 股规则是执行准入合同，不能视为已实现的撮合能力。

### 账户归档及配置换版记录

| 入库表 | 字段及约束 | 用途 | 生命周期 |
| --- | --- | --- | --- |
| paper_strategy_event | id 主键、strategy_id 外键、kind 限 archive/restore/reconfigure、payload JSON、created_at；strategy_id/created_at 索引 | 保存归档、恢复和配置换版事件；payload保留新旧workspace_id，旧配置与报告不可删除 | 长期审计记录，不自动清理 |

账户归档状态由最近archive/restore事件推导；配置换版原子创建新研究配置并更换账户workspace_id，保留原资金流水，不重新注资。归档账户禁止换版。归档并非停止运行中的撮合（当前撮合未实现）。

### 账户执行准入与账本（2026-10-26）

生产仅检查准入并保存原因，未调用撮合，未启用自动交易。成交记账内核仅经固定输入验证，不等于上线成交能力。详见 `docs/research/account-execution-ledger.md`。

| 入库表 | 字段和约束 | 用途 | 生命周期与清理保护 |
|---|---|---|---|
| execution_run | id 主键；account_id、workspace_id 外键；analysis_id 可空外键；request_key、rules_version、context、created_at 非空；UNIQUE(account_id,request_key)；账户/时间索引 | 冻结当前配置、研究报告引用、报价/日历观察、缺失能力及费用版本；同请求不重复执行 | 长期审计；禁止TTL、归档删除或缓存清理 |
| execution_order | id主键；run_id/account_id外键；symbol、side(buy/sell/hold)、quantity>=0、可空price_fen、status(filled/rejected/hold)、reason、created_at；UNIQUE(run_id,symbol)；账户/时间索引 | 检查或平台订单记录；检查失败数量0、价格空，不能视作成交 | 长期；禁止自动清理 |
| execution_fill | id主键；order_id唯一外键、account_id外键；symbol、side(buy/sell)、quantity>0、price_fen>0、fee_fen>=0、trade_date、created_at；账户/交易日索引 | 平台成交事实；每订单最多一次成交；当前生产不产生记录 | 长期账本；禁止自动清理或回放重写 |
| execution_cash | id主键、account_id外键、fill_id唯一外键、非零amount_fen、created_at | 成交引起的现金变动；与paper_cash_ledger初始入金合计确定可用现金 | 长期资金账本；禁止自动删除 |
| execution_lot | id主键；account_id外键；symbol；fill_id唯一外键；trade_date、quantity>0、remaining在0到quantity之间、cost_fen>0；账户/股票/日期索引 | 买入批次与FIFO剩余量、T+1可用数；原始成交和现金不覆写 | 长期；零剩余批次亦保留；只由平台事务更新remaining |
| execution_nav | id主键；run_id唯一外键、account_id外键；cash_fen>=0；market_fen/equity_fen可空且>=0；created_at | 检查时账户估值；持仓缺少可信估值时保持null；不是日频净值或收益曲线 | 长期账户记录；禁止TTL清理 |

所有金额为人民币分。六表纳入备份、恢复、schema探测和保留政策注册。检查/配置引用不随缓存删除；没有行情或已审核执行策略时不得虚构成交。新增任何生产执行入口仍需独立准入和幂等验收，Agent不得直接写入这些表。

<!-- persistence-schema:start -->
政策文件 SHA256：`b71395fb08ebae648b5cd92d4bec1fdb9afae76d28fc86948cf2a0dc532e98c1`

| 实际表名 | 实际字段（迁移完成后） | schema SHA256 |
|---|---|---|
| app_setting | key TEXT, value TEXT | `be4e37de8b0f8f2691badd8e7d1bce793364b1ca7cfb16c6b59a912a2101bf1e` |
| backfill_audit | id INTEGER, dataset TEXT, start_date TEXT, end_date TEXT, changed INTEGER, operator TEXT, created_at TEXT | `1c48bb9d490403c7db5590bec23e5decfd9a63bb96b99c3331e259709e30e3f0` |
| bar_cache | symbol TEXT, period TEXT, bucket_start INTEGER, source TEXT, payload TEXT, observation_hash TEXT, revision INTEGER, observed_at INTEGER, last_accessed_at INTEGER | `e0c834e85cc6fa0f2a748e2ebed5bb7a0d45f1839a0737832aed40a09f32a64a` |
| cache_tombstone | id TEXT, period TEXT, hash TEXT, size_bytes INTEGER, deleted_at INTEGER, reason TEXT, actor TEXT | `e63dc4c2f4e6af6d65af18fe94629b527463a95928af27443e48bee44f0278f7` |
| cleanup_audit | id INTEGER, plan_id TEXT, table_name TEXT, record_key TEXT, deleted_rows INTEGER, created_at INTEGER | `fabcf18f244c4627f8350632645ab276b21b98552e842b4851eb8cb41d2ad976` |
| cleanup_plan | id TEXT, policy_version INTEGER, created_at INTEGER, expires_at INTEGER, candidates TEXT, status TEXT | `611081e85eb8fed5e6c389a6daf78349c743284abb71f8908b1ede77f5fd7624` |
| company_profile | symbol TEXT, full_name TEXT, business TEXT, industry TEXT, list_date TEXT, controller TEXT, website TEXT, region TEXT, intro TEXT, source TEXT, updated_at TEXT | `8e4b2d017889f717beab7365a815d0e23e14f2db34051ce92328a008751acd25` |
| daily_basic | symbol TEXT, trade_date TEXT, total_shares REAL, float_shares REAL, total_mv REAL, float_mv REAL, pe_ttm REAL, pb REAL, turnover REAL | `dc775a52d2b2adf62e2c098b202eb1cefd32b6bb31ced66454f96d31e4cab9e8` |
| data_freshness | dataset TEXT, status TEXT, last_success_at TEXT, last_source TEXT, last_batch_id TEXT, last_error TEXT, updated_at TEXT | `cfb7e55381c078c050548dde04e92fa9b3dc74a0d63da9a34656064e80c552b1` |
| data_job | id TEXT, dataset TEXT, status TEXT, progress INTEGER, message TEXT, started_at TEXT, finished_at TEXT, idempotency_key TEXT, attempt INTEGER, updated_at TEXT | `ce884c33d204f41683d1cf45d7c94c98ea692325ff0c5b636caa5fab7f755a8c` |
| data_reference | owner TEXT, input_hash TEXT, created_at INTEGER | `50b11806775422fa58255f93dbc53245d68ad350ed8777d16c82260154f6365b` |
| data_source_config | id TEXT, provider TEXT, enabled INTEGER, base_url TEXT, secret_env TEXT, updated_at TEXT | `45865825fbf188bf849327bc6fb12a4ed441bc23a3e1d38848deaff4f6c6d2dc` |
| execution_cash | id TEXT, account_id TEXT, fill_id TEXT, amount_fen INTEGER, created_at TEXT | `54a0431b7be124332af3411ac26a4387b22e36f8c2b86cff1c18e98cb36be06e` |
| execution_fill | id TEXT, order_id TEXT, account_id TEXT, symbol TEXT, side TEXT, quantity INTEGER, price_fen INTEGER, fee_fen INTEGER, trade_date TEXT, created_at TEXT | `f02738d12fdb9805666aa962e193a2bd11b45c41a5981bc6b929235d9a081ec3` |
| execution_lot | id TEXT, account_id TEXT, symbol TEXT, fill_id TEXT, trade_date TEXT, quantity INTEGER, remaining INTEGER, cost_fen INTEGER | `b9c1ef4420a99bc0ab862b9a721aa6f80bb827ff41403a3ecaae44da0f4a0720` |
| execution_nav | id TEXT, run_id TEXT, account_id TEXT, cash_fen INTEGER, market_fen INTEGER, equity_fen INTEGER, created_at TEXT | `6ab025e2094531fc9ffc6f563ad019b38988a38fee54136314950b49a6e72998` |
| execution_order | id TEXT, run_id TEXT, account_id TEXT, symbol TEXT, side TEXT, quantity INTEGER, price_fen INTEGER, status TEXT, reason TEXT, created_at TEXT | `6d2f7a56eb2930fc87f7212cc1f209cfda1ccb71ddc3697e00d295d503626d3b` |
| execution_run | id TEXT, account_id TEXT, request_key TEXT, workspace_id TEXT, analysis_id TEXT, rules_version TEXT, context TEXT, created_at TEXT | `39444ac5f73037eb5325dba4ff25e2cbb3dddf3e8a0ca2dc2ce32f466dc3c33f` |
| financial_indicator | symbol TEXT, report_period TEXT, roe REAL, gross_margin REAL, net_margin REAL, debt_ratio REAL, eps REAL, bps REAL | `2d3ae478d4af1de94cb3eabebec5ed0af37a3af1557928d86ca2a5d5ca1e0128` |
| financial_report | symbol TEXT, report_period TEXT, announce_date TEXT, statement_type TEXT, version INTEGER, effective_at TEXT, supersedes INTEGER, payload_hash TEXT, is_current INTEGER, revenue REAL, net_profit REAL, total_assets REAL, total_liabilities REAL, equity REAL, operating_cash_flow REAL, collected_at TEXT | `3b4aa64ac9e3741737d9bdf66f02495aa6871bfd2676f24fba21fc27c7b7854b` |
| index_catalog | code TEXT, name TEXT, market TEXT, category TEXT, status TEXT | `16aa615e4a6b11e3c28fefb50c5048722a82300078d9a729b3a56746c1170f1c` |
| index_member | index_code TEXT, symbol TEXT, in_date TEXT, out_date TEXT, weight REAL | `a4b81aac46d5f71f6238b2e744af72f3d985121326af0aa8bed416585c116e4d` |
| index_quote | index_code TEXT, trade_date TEXT, open REAL, high REAL, low REAL, close REAL, volume REAL, amount REAL, source TEXT, batch_id TEXT, fetched_at TEXT, amount_known INTEGER | `09ed2699a3d83f6fa901a6f5639ed07dbcf1d523422f5a6fbd861f218ede5f46` |
| input_manifest | hash TEXT, relative_path TEXT, size_bytes INTEGER, source TEXT, available_at TEXT, created_at INTEGER, pinned INTEGER | `764b7e73d1b8461e492b4bcfeb7cfb5cc45240d087474ba67265177f784e1922` |
| instruments | symbol TEXT, name TEXT, pinyin TEXT, exchange TEXT, board TEXT, industry TEXT, list_date TEXT, delist_date TEXT, status TEXT | `8c4c7c28c2dc7330c4a113f4e1f556cdf417cfd9a67e799495533a56f40fca15` |
| job_idempotency | key TEXT, job_id TEXT, result TEXT, expires_at INTEGER | `f9506c1e431308684093d38fbca9a53d7db406fbf32c3b868fd4bfd25dd57eba` |
| lifecycle_policy | dataset TEXT, policy_version INTEGER, config TEXT, updated_at INTEGER | `2704be39777bcc5c051292265b11b80fd97a8fbd6f587d959fb2091b240c95bc` |
| live_cache | symbol TEXT, dataset TEXT, payload TEXT, source TEXT, fetched_at INTEGER, attempted_at INTEGER, failures INTEGER, error TEXT, last_accessed_at INTEGER, policy_version INTEGER | `45997667480ac3879bd51b2192fb8e3c6ae72fe5d4f4235540b92401740d2e66` |
| market_breadth_observation | id INTEGER, trade_date TEXT, source TEXT, scope TEXT, provider_time TEXT, observed_at INTEGER, available_at INTEGER, phase TEXT, payload TEXT, observation_hash TEXT | `9a2a03f7b16c94a3e38a1ae08e6df834a50a399133df7e506992f5c4d16705ec` |
| market_snapshot | trade_date TEXT, payload TEXT, computed_at TEXT | `ca18dff57ec00ec2d5548e11b00af0c392f2f1281d1ec6946e76f8d2b5c26f98` |
| paper_cash_ledger | id TEXT, strategy_id TEXT, amount_fen INTEGER, kind TEXT, created_at TEXT | `867a3bb50cc353aa4c8f01643c0736817ca5891aecce8c9aaef8be4814f31dd7` |
| paper_strategy | id TEXT, request_key TEXT, workspace_id TEXT, initial_cash_fen INTEGER, policy TEXT, created_at TEXT | `ee2cc09631cfdbb0ef4d4ec6364c7d81dfef85068c509201c69454c06d609394` |
| paper_strategy_event | id TEXT, strategy_id TEXT, kind TEXT, payload TEXT, created_at TEXT | `9712a1e33c7bdd027579582bf2ca247c53b25950cf32e5ac2983867a96f95ac2` |
| quality_report | dataset TEXT, coverage REAL, missing INTEGER, duplicates INTEGER, anomalies INTEGER, updated_at TEXT | `769bbbaceb96e94be35108a8aeae711cc530c63ff000dc0a068251db9ab8fb81` |
| quant_monitor | id TEXT, name TEXT, family TEXT, definition TEXT, enabled INTEGER, created_at TEXT | `9b8cc2060de7fa5c2a7c0812283f15610d13bff9fd1a29650cd9d96415e775b9` |
| quant_observation | id TEXT, monitor_id TEXT, symbol TEXT, as_of TEXT, input_hash TEXT, payload TEXT, created_at TEXT | `1e4ba5e1f8c297991f0c6b00563afbc05a1defcb8aaee9ea49abcccd1e2a25bc` |
| quote_daily | symbol TEXT, trade_date TEXT, open REAL, high REAL, low REAL, close REAL, volume REAL, amount REAL, adj_factor REAL, turnover REAL, source TEXT, batch_id TEXT, fetched_at TEXT, amount_known INTEGER, adj_source TEXT, adj_batch_id TEXT | `6b8456b6fb9d0851214463f3907be386d146df7ee298603ff4deabe79ddb6cec` |
| quote_snapshot | symbol TEXT, name TEXT, trade_date TEXT, open REAL, high REAL, low REAL, close REAL, prev_close REAL, volume REAL, amount REAL, change_pct REAL, source TEXT, batch_id TEXT, fetched_at TEXT, observation_hash TEXT, provider_time TEXT, last_checked_at TEXT | `883134f14fc67a793be2cb9684a357750b76789d7427b365bc8ef486e32421db` |
| quote_staging | id INTEGER, symbol TEXT, trade_date TEXT, open REAL, high REAL, low REAL, close REAL, volume REAL, amount REAL, adj_factor REAL, status TEXT, reason TEXT, batch_id TEXT, ingested_at TEXT | `b706b1c4195d35109b49edc0d5344a1177b55940bf5862a17b716a9572a281f3` |
| reference_correction | id INTEGER, dataset TEXT, record_key TEXT, field TEXT, old_value TEXT, new_value TEXT, operator TEXT, corrected_at TEXT | `1c537e35d6355a6b0fc31405ec0eba55f7b5f346a3f761d36ba045f281cb18a8` |
| research_analysis | id TEXT, workspace_id TEXT, input_hash TEXT, input TEXT, report TEXT, created_at TEXT | `5a9e32e0bd09965dfc037ec2aadc0850802ea003c0a321c9b0bbc49a9f0c503c` |
| research_workspace | id TEXT, config TEXT, created_at TEXT | `81060a40223a6b522a2f03c380adf42427e1a839c59bf49bc99c55e2e2ce0890` |
| sector_catalog | id INTEGER, name TEXT, sector_type TEXT, source TEXT, status TEXT | `55dfe9301c23d1c7ed09b422742f0fd4549fd5f1954d5926243dcfa5e63bbe8e` |
| sector_member | sector_id INTEGER, symbol TEXT, in_date TEXT, out_date TEXT, weight REAL | `71790aa871c958638596695eefb532d52b5babb0f1bdd5e9b2a524808f58e847` |
| source_retry_task | dataset TEXT, symbol TEXT, start_date TEXT, end_date TEXT, attempts INTEGER, last_error TEXT, created_at TEXT, updated_at TEXT | `ebc2f983c295b7a1dab7609aad99a57af7d4c6f8fd5a0c61ebed439006377d60` |
| subscription_lease | client_id TEXT, symbol TEXT, dataset TEXT, expires_at INTEGER | `63102fa0e8220b2134246aae80a5cc108c84da8cd9f088e39b80cc0e21c01b57` |
| sync_checkpoint | dataset TEXT, checkpoint TEXT, updated_at TEXT | `511f10ef7594c9e10e0d503c1f9032a371a0ffdf418e3cdc2eecebbcd0e86c2a` |
| trading_calendar | trade_date TEXT, is_open INTEGER | `9b0fd8fce64223aa7ddfdfe7761e8ba38746a9f5fdfa90e76ecd44cedf870f22` |
| watch_alert | id TEXT, symbol TEXT, kind TEXT, threshold REAL, cooldown_minutes INTEGER, enabled INTEGER, last_triggered_at TEXT, last_observation TEXT, created_at TEXT | `e1b42afe5c1ccad358fbbd3fad214aeefbc1112525a366c7df009525b05e787d` |
| watch_alert_event | id INTEGER, rule_id TEXT, symbol TEXT, message TEXT, as_of TEXT, created_at TEXT | `034327296901f13d0b79fadd15efa4d0b32df9d201f7c1fcd6d171347a1ebf26` |
| watchlist_group | id TEXT, name TEXT, sort INTEGER, color TEXT | `80d435abdb631dd908099b36965fb066c4384aa06c763de8fb4b1c1306261205` |
| watchlist_item | id TEXT, group_id TEXT, symbol TEXT, note TEXT, sort INTEGER, created_at TEXT | `b8b7815fb2cca56750cb394ef961c32f4e44c7d117f19e9693668a89e100fabc` |
<!-- persistence-schema:end -->


备份资产：`*.db`、`*.db.sha256`、`*.db.manifest.json` 和 `*.db.inputs/` 必须一起保管。凭据不打包；外置输入默认位于数据库同名 `.inputs` 目录，可用 `ALPHAK_INPUT_DIR` 指定。迁移/恢复前备份不自动轮换，需用户显式管理。

### 2026-10-25 数据源补充与样本集合初始化

不新增或改变表结构、索引、约束、留存期限与清理资格。`market_breadth_observation` 增加 AxData 通达信沪深聚合历史的来源值（30m/60m/day），只合并相同时刻，不采集全市场个股；`payload.flat_known=false` 表示上游不提供平盘数。财联社当前快照无上游日期，只进入既有 `live_cache`，不以采集时间冒充历史。`watchlist_item` 补入本地最新可比样本，既有分组/备注保留；既有 `app_setting` 保存一次性迁移标记 `sample.collection.v1`，随配置备份、长期保存，不参与缓存清理。
