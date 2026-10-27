# AlphaK 量化研究文档

本目录是研究设计，不是已经运行的评分或策略服务。

1. [量化研究体系](../quant-research-system.md)：五层职责、数据PIT、双评分、情绪与预期、生命周期和工程边界。
2. [文献与证据登记](quant-baseline-literature.md)：18项文献、访问等级、采用决策和排除记录。
3. [基准数学模型](quant-baseline-model.md)：情绪代理、条件模型、事件分析、风险与样本外实验设计。
4. [B/S与目标仓位合同](strategy-decision-contract.md)：准入、触发、退出、情绪状态机、优先级、风险预算及执行边界。
5. [文献核验记录](literature-verification.json)：在线书目元数据及固定原文版本访问摘要。
6. [数学校核脚本](verify_baseline_math.py) 与 [结果](baseline-math-check.json)：构造示例，非行情验证。

先冻结池与数据能力，再预注册参数、做PIT与执行校验，最后才能运行真实数据实验。任何模型改动必须新版本；不得把论文里的成绩当成本平台成绩。

- [策略实验与自动调优闭环](strategy-auto-tuning.md)：先跑技术基线、54组有限搜索、时间验证、候选选择、影子运行与晋升边界。

- [指标与情绪策略模型目录](quant-model-catalog.md)：24 项指标、8 个策略草案，逐项解释情绪参与、学习、进出场、仓位与数据门槛。

- [实盘研究配置与历史分析](trading-analysis-workspace.md)：当前可用的配置、分析接口、试验适配器、算法及未实现能力。

- [量化模型计算与上下文契约](quant-engine-calculation-contract.md)：8个模型的输入、公式、建模、策略整合、上下文流转与实现缺口；[结构化目录](../specs/research/model-calculation-contracts.json)。

- **[八个模型独立完整文档](models/README.md)**：逐模型数学推导、特征/数据处理、训练调优、B/S与仓位、情绪与收敛分析；附[当前实现一致性核对及补充清单](models/implementation-review.md)。版本design-2026-10-26.1，明确区分当前试验与未实现方案。
