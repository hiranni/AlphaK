# 量化模型计算与上下文契约

日期：2026-10-26；版本：research-contract-1；范围：≤8只股票的研究分析，无全市场扫描，无真实下单。

本文件补充模型目录中的研究叙述，规定引擎阶段、字段和计算责任。逐模型公式、输入字段、建模步骤、门槛、组合规则和上下文见 [结构化契约目录](../specs/research/model-calculation-contracts.json)。页面通过同内容的 `quantEngineContracts.ts` 展示。该目录是声明式计算规范，**不是已经接入运行器的DSL**；现有实现为 `apps/api/src/trading_research.rs`，未实现模型明确blocked。

## 1. 数据依赖及连接规则

| 输入 | 连接键 | 时间约束 | 质量/缺失规则 | 使用模型 |
| --- | --- | --- | --- | --- |
| 日线OHLCV | symbol,date | 已完成日线；北京时间15:10前截止前一日 | 最多400条、至少180有效条；剔除demo/seed/synthetic及无效OHLCV；行情源价格/复权须一致 | 全部 |
| 池内上涨比例 | date,scope=pool | 同日期股票完整 | B=上涨股票数/N，平盘计入分母；不是大盘情绪 | 前5及未来组合 |
| 大盘历史情绪 | date,scope=market | 当时已公开的聚合记录 | 当前未适配，market模式blocked；不以小池替代 | 目标市场环境 |
| 公告事件与预期 | symbol,event_id,available_at | published_at与available_at均不晚于决策 | 文本哈希去重；缺历史一致预期不能计算超预期 | Event（待接） |
| 财报版本 | symbol,report_period,revision_id | 按公开时间as-of join，不能按报告期末回填 | 原版、修订版分别保存；行业不适用指标为null | Quality（待接） |
| 历史估值/行业参考分布 | symbol,date,industry,reference_version | 基准统计同样PIT | 缺失不做权重重归一；不触发实例全市场扫描 | Quality（待接） |
| 子模型OOS预测 | model_version,symbol,decision_at,horizon | 预测生成时不得看到标签；标签成熟后才训练组合 | 期限对齐，缺失不填0；高度相关适配器不当独立证据 | Ensemble（待接） |
| 账户/持仓风险 | instance_id,as_of | 交易决策前获取并冻结 | 与研究配置分离；当前没有接入此链路 | 后续风险与模拟执行 |

当前有效观测步可能跨缺失交易日，5步标签不保证5个连续交易日。价格复权一致性、公司行动、完整交易日历仍需在正式回测前补齐；不能用当前试验结果宣称可执行收益。

## 2. 分层运行接口

| 阶段 | 输入 → 输出 | 计算/责任 | 失败行为 |
| --- | --- | --- | --- |
| FreezeConfig | 配置 → workspace_id/config_hash | 股票唯一且1–8；策略已知；权重>0且总和≤1；模型不绑定Agent | HTTP400，不开始分析 |
| LoadSnapshot | 配置+cutoff → InputSnapshot | 最多8×400日线；冻结来源、日期、采集时间及原值 | 数据不齐或过期blocked |
| BuildFeatures | InputSnapshot → FeatureFrame | 按契约7维公式，池内宽度按同日计算；先校验再计算 | null/blocked，不编造值 |
| FitValidate | FeatureFrame+成熟标签 → Forecast | 训练尺度只用历史；18组时间验证；保存全部试验 | 样本不足blocked |
| ApplyModel | Forecast+模型门槛 → ResearchDecision | 逐模型独立输出买入候选/减仓复核/等待 | 不强行产生买入 |
| Allocate | ResearchDecision+资金权重 → 研究上限 | min(20%,80%/N,0.5%/下行)×clip(edge/下行,0,1)，再乘策略资金权重 | 防守/等待上限0并非卖出指令 |
| FreezeReport | 全链路 → research_analysis | 配置、原始输入、参数、来源和代码指纹冻结，相同输入复用 | 不写订单、不更新净值 |
| FutureExecution | 研究候选+账户 → 校验后的模拟指令 | 待实现：跨模型按股票净额→组合风控→现金/整手/T+1/涨跌停/流动性→模拟撮合 | 任一不通过不下单；Agent不能写订单表 |

当前5个试验适配器共享预测器，区别在固定策略门槛，不能称为5个独立训练模型。Event/Quality/Ensemble的设计公式和样本门槛为待验证规范，不是已完成代码。

## 3. 模型计算索引

| 模型 | 当前/目标建模方法 | 专属判断 | 当前状态 |
| --- | --- | --- | --- |
| State | 标准化7维状态近邻估计条件收益 | 池内上涨≥25%+edge与历史验证通过 | 试验适配 |
| Regime | 当前近邻+门槛；目标Gaussian HMM前向过滤 | 上涨≥50%且MA20偏离>0；不使用全序列平滑状态 | HMM未实现 |
| Continuation | 近邻条件收益+量价情绪确认 | 突破前20日最高价，量比≥1.2，上涨≥50% | 试验适配 |
| Repair | 近邻条件收益+恐慌修复过滤 | 60步回撤<−8%、宽度改善、当日反弹 | 试验适配 |
| Cooling | 拥挤/退潮门槛；只减仓复核 | 宽度下降、MA20偏离>5%、量比>1.5 | 试验适配，无持仓执行 |
| Event | 事件超预期、事件前β估计异常收益、岭回归 | 日期分块，去事件簇泄漏，成本后预测 | 待实现 |
| Quality | PIT行业适配因子、质量与机会双维 | 非负岭回归权重候选对照固定权重；财务硬风险独立 | 待实现 |
| Ensemble | 成熟OOS子预测的约束回归与漂移惩罚 | 非负权重、总和≤1、用户预算硬上限，留现金 | 待实现 |

所有模型的逐字段公式及训练窗口在JSON目录中。目标算法的60样本、质量70分等门槛均为待验证设计参数，禁止包装为已验证最优参数。

## 4. 上下文流转与所有权

`配置 → InputSnapshot → FeatureFrame → Forecast → ResearchDecision → RiskReview → 冻结报告`。

每阶段传递不可变引用：workspace_id、as_of、input_hash、模型版本、规则版本、代码指纹；字段载荷必须能追回原始输入。预测增加训练窗口、标签成熟时间、标准化尺度、候选参数评分、相似样本；决策增加命中/未命中门槛、理由、风险、有效期和研究仓位上限。目标中的RiskReview不得偷读别的实例持仓。

当前API保存输入、特征、训练均值及预测/试验结果，但没有独立的FeatureFrame表、完整标准化尺度/逐门槛布尔轨迹字段，也没有RiskReview执行服务。它们是下一步实现差距，不能声称所有中间契约已经实现。现有冻结输入和代码指纹允许按固定版本复算；回放必须只读。

未来Agent上下文由运行时构建，Agent只输出语义响应；运行时注入envelope并通过schema及业务校验，禁止Agent直接操作订单表。策略定义不声明Agent，绑定归实例。研究报告不等于strategy-instance执行契约。

## 5. 可验收条件

1. 8个模型均能查到输入字段、公式/建模、专属门槛、整合步骤和上下文责任；已实现与计划明确区分。
2. 未来价格/财报修订不得改变历史时点预测；标签必须在决策前成熟。
3. pool/market情绪不得互换，未知不填0，必需依赖缺失返回blocked。
4. 配置与报告不可覆盖，原始输入、模型版本、参数及验证结果可追溯。
5. 组合不能把训练内预测当OOS数据，调参后必须另存版本；正式上线前补独立测试和成本稳健性验证。
6. 当前交付不新增持久化表或订单行为；若后续新增字段/表/保留策略，必须同步持久化提案及harness。

## 6. 机器字段与操作图

结构化目录除展示文本，还提供 `engine_pipeline`（输入边界、7个feature算子、label偏移、估计器参数网格、验证目标、allocation公式、阶段reads/writes/失败策略）与每个模型的 `runtime_binding.gate_ast`（all/gt/gte/lt规则树）。数值阈值与操作不依赖引擎从中文说明中提取。当前运行器仍为Rust硬编码实现，JSON尚未被解释执行，后续接入必须先做算子allowlist、版本/schema校验与等价性fixture，禁止直接eval公式字符串。设计公式里的字符串只作规范表达，不能作为不可信代码执行。
