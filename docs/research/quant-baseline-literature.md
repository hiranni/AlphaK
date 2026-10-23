# AlphaK 基准模型文献与证据登记

检索日期：2026-10-22。用途：为 A 股、日频、人工固定小股票池的研究模型选方法，而不是证明 AlphaK 已经有超额收益。配套数学方案见 [基准模型](quant-baseline-model.md)。

## 检索与证据边界

本轮通过 Crossref 在线查询 DOI、作者、题名和可用摘要，并访问 NBER、AEA、arXiv 的原始论文页面；对两篇近年 LLM 论文读取了固定版本 HTML 正文的研究方法与限制。检索组合包括 investor sentiment / Chinese stock market、financial LLM / asset pricing、factor investing / backtest overfitting，以及已知核心文献的题名检索。覆盖经典方法与2023–2026检索结果，不是系统综述，也不声称穷尽当前论文。

证据等级：A=读取固定版本原文相关章节；B=读取原始摘要或作者/机构研究摘要；C=仅核对书目元数据，作为方法出处，不据此转述未阅读的实验细节。同行评审与访问等级是两回事；工作论文的摘要和正文不能视为已复现结论。

2026 年检索命中的若干 SSRN 记录只有年份、未核实具体发布日期和全文，因此不纳入模型依据。Crossref 的元数据可能更新，摘要不一定对应最初投稿版本；下表显式区分工作论文、期刊和固定 arXiv 版本。所有收益、准确率都属于原研究环境，不能迁移成 AlphaK 的承诺。

## 核心文献及采用决策

| ID | 文献与可访问来源 | 访问/状态 | AlphaK 采用与限制 |
| --- | --- | --- | --- |
| R01 | Fama & French (2015), *A five-factor asset pricing model*, JFE. [DOI](https://doi.org/10.1016/j.jfineco.2014.10.010) | C；期刊 | 价值、盈利和投资等概念分开研究；不把解释资产收益的因子模型直接称为个股评分或买入模型，不在8只股票内复制全市场因子组合。 |
| R02 | Novy-Marx (2013), *The other side of value: The gross profitability premium*, JFE. [DOI](https://doi.org/10.1016/j.jfineco.2013.01.003)；[NBER工作论文](https://www.nber.org/papers/w15940) | B（NBER摘要）；期刊书目已核对，工作论文题名略有不同 | 盈利质量与估值分开；毛利/资产不是ROE的同义词。文献不能证明我们的质量权重。 |
| R03 | Liu, Stambaugh & Yuan (2018工作论文，2019期刊版本), *Size and Value in China*. [NBER w24458](https://www.nber.org/papers/w24458)，[DOI](https://doi.org/10.3386/w24458) | B；读取NBER摘要及发表版本说明 | 中国市场制度和壳价值等会影响因子构造；不机械移植美股因子。原文的剔除最小30%规则不是本项目默认规则，也不授权全市场扫描。 |
| R04 | Moskowitz, Ooi & Pedersen (2012), *Time series momentum*, JFE. [DOI](https://doi.org/10.1016/j.jfineco.2011.11.003) | C；期刊 | 作为时间序列动量方法出处；其跨资产研究不能直接证明A股20/60日规则有效。 |
| R05 | Baker & Wurgler (2006), *Investor Sentiment and the Cross-Section of Stock Returns*, JF. [DOI](https://doi.org/10.1111/j.1540-6261.2006.00885.x) | B；Crossref提供原始摘要 | 摘要报告情绪影响具有异质性，估值主观、套利困难证券更敏感；支持情绪×股票特征的假设，不支持情绪单调加分。 |
| R06 | Baker & Wurgler (2007), *Investor Sentiment in the Stock Market*, JEP. [AEA](https://www.aeaweb.org/articles?id=10.1257/jep.21.2.129)，[DOI](https://doi.org/10.1257/jep.21.2.129) | B；原始摘要 | 情绪与套利限制结合；我们的涨跌分布代理不是原论文情绪指数，不能沿用其名称冒称复现。 |
| R07 | Tetlock (2007), *Giving Content to Investor Sentiment: The Role of Media in the Stock Market*, JF. [DOI](https://doi.org/10.1111/j.1540-6261.2007.01232.x) | B；原始摘要 | 摘要报告媒体悲观与价格压力、随后反转及交易量的关系；为消息和市场反应分离提供动机，非中文新闻因果证明。 |
| R08 | Loughran & McDonald (2011), *When Is a Liability Not a Liability? Textual Analysis, Dictionaries, and 10-Ks*, JF. [DOI](https://doi.org/10.1111/j.1540-6261.2010.01625.x) | B；原始摘要 | 通用负面词在财务文本会误分类；中文金融词典/模型需本地标注校验，不能直接翻译英文情绪词表。 |
| R09 | Hamilton (1989), *A New Approach to the Economic Analysis of Nonstationary Time Series and the Business Cycle*, Econometrica. [DOI](https://doi.org/10.2307/1912559) | C；期刊 | 马尔可夫状态切换方法出处；仅作为后续对照。实时判断必须用过滤概率，不能使用看到未来的平滑概率。 |
| R10 | Ledoit & Wolf (2004), *A well-conditioned estimator for large-dimensional covariance matrices*, JMA. [DOI](https://doi.org/10.1016/S0047-259X(03)00096-4) | C；期刊 | 协方差收缩的方法出处；小池也需控制估计误差，但不等于该方法必然提高收益。 |
| R11 | Newey & West (1987), *A Simple, Positive Semi-Definite, Heteroskedasticity and Autocorrelation Consistent Covariance Matrix*, Econometrica. [DOI](https://doi.org/10.2307/1913610) | C；期刊 | 时间相关与异方差稳健推断的出处；HAC不能修复未来数据泄漏或缺失交易成本。 |
| R12 | White (2000), *A Reality Check for Data Snooping*, Econometrica. [DOI](https://doi.org/10.1111/1468-0262.00152) | C；期刊 | 多次试验选择偏差的方法出处；必须保留全部试验而非只报告最佳规则。V1先做预注册与配对区块自助法，不冒称已实现Reality Check。 |
| R13 | Harvey, Liu & Zhu (2016；在线元数据2015), *… and the Cross-Section of Expected Returns*, RFS. [DOI](https://doi.org/10.1093/rfs/hhv059) | C；期刊 | 多重检验文献出处；不把惯用t值当作本项目的自动发布门槛。 |
| R14 | Bailey & López de Prado (2014), *The Deflated Sharpe Ratio: Correcting for Selection Bias, Backtest Overfitting, and Non-Normality*, JPM. [DOI](https://doi.org/10.3905/jpm.2014.40.5.094) | C；期刊 | DSR作为后续诊断候选；须有试验数、相关结构和分布假设，不能用普通Sharpe替代或伪造DSR数值。 |
| R15 | Gu, Kelly & Xiu (2020), *Empirical Asset Pricing via Machine Learning*, RFS. [DOI](https://doi.org/10.1093/rfs/hhaa009) | B；原始摘要 | 摘要强调非线性交互和动量/流动性/波动特征；仅支持检验交互的动机。其大样本结果不支持8只股票上直接训练大型模型。 |
| R16 | Yang, Liu & Wang, *FinGPT: Open-Source Financial Large Language Models*. [固定v2摘要](https://arxiv.org/abs/2306.06031v2)，[v2正文](https://arxiv.org/html/2306.06031v2) | A；2023首发、2025-11-15修订；框架/实验论文 | 参考文本采集、版本与领域适配设计；文本分类实验不等于A股策略盈利证明，V1不依赖训练大模型。 |
| R17 | Lopez-Lira & Tang, *Can ChatGPT Forecast Stock Price Movements? Return Predictability and Large Language Models*. [固定v6](https://arxiv.org/abs/2304.07619v6)，[v6正文](https://arxiv.org/html/2304.07619v6)；[期刊DOI](https://doi.org/10.1016/j.jfineco.2026.104335) | A（arXiv v6）；2023首发，2025-10-28修订；期刊Crossref记录为2026-10，未核验期刊全文与v6一致性 | 原文区分不可交易的即时反应与随后漂移，讨论成本和模型知识截止；不能将约90%的即时反应识别率称为下一日个股预测准确率。美股多空结果不能直接迁移A股多头策略。 |
| R18 | Schuettler, Audrino & Sigrist (2024), *Does sentiment help in asset pricing? A novel approach using large language models and market-based labels*. [SSRN DOI](https://doi.org/10.2139/ssrn.4905533) | B；工作论文，摘要未独立复现 | 摘要称收益主要来自空头腿；A股V1不支持做空，故只列研究候选，不采用其收益结论。使用市场结果标注必须隔离标签窗口，避免预测目标泄漏。 |

## 被排除与未完成核验

- 初始候选 `10.1016/j.jfineco.2018.11.001` 实际为 *Policy externalities and banking integration*，不是 *Size and Value in China*，已剔除并改用核实后的NBER记录。
- 候选 arXiv `1407.2588` 实际为组合数学论文，与回测过拟合无关，已剔除；候选 DOI `10.21314/jcf.2017.322` 返回404，不作为依据。
- 2026年SSRN搜索结果中仅有年份、未确认具体日期/正文的条目不作为“最新有效证据”。本轮最新可读固定版本为上述2025修订稿；2026期刊记录仅验证书目信息。
- 部分Crossref请求限流，已改为低并发并重试关键条目。元数据可访问不代表期刊全文已读。

## 对建模的结论

1. 双评分是产品研究表示法，既不是Fama–French因子组合，也不是经文献证明的收益预测器。
2. 情绪不能作为一个永远正向的加分项；必须区分方向、活跃度、分歧和拥挤风险，并检验与动量的重复。
3. V1最合适的起点是可解释固定规则与低维收缩模型的分层对照，复杂隐状态/文本模型放在后续挑战者。
4. 论文支持的是检验假设和控制偏差的方法，AlphaK的可用性必须依赖自身PIT数据、A股执行约束、样本外与前向记录验证。

原始检索元数据与访问校验摘要保存在 [文献核验记录](literature-verification.json)，不保存或分发受限期刊全文。
