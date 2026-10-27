/** Versioned research contracts. Declarative documentation, not an execution DSL. */
export type EngineContract = { status: string; inputs: [string,string,string][]; features: string; fit: string; gate: string; integration: string; context: string };
const bars: [string,string,string][] = [
 ['daily_bars','symbol,date,open,high,low,close,volume,source,fetched_at','已收盘日线；最多400根、至少180根；同一价格口径，剔除演示及无效OHLCV'],
 ['pool_breadth','date,up_count,total_count,scope','仅实例内≤8只股票，同日齐备；Bₜ=上涨数/N，平盘计入分母'],
 ['research_config','symbols,model_id,weight,cutoff','不可变配置；权重>0、总和≤1；北京时间15:10之前不使用当日日线'],
];
const features = 'xₜ=[Cₜ/Cₜ₋₅−1, Cₜ/Cₜ₋₂₀−1, Cₜ/mean(Cₜ₋₁₉:ₜ)−1, max(sd(rₜ₋₁₉:ₜ),0.003), Vₜ/mean(Vₜ₋₂₀:ₜ₋₁), Cₜ/max(Cₜ₋₅₉:ₜ)−1, Bₜ]；rₜ=Cₜ/Cₜ₋₁−1。单位为小数收益、倍数、比例；量均值为0时当前适配器返回量比0。';
const fit = '标签 yⱼ=Oⱼ₊₆/Oⱼ₊₁−1，仅接纳 j+6<t 的成熟标签；有效观测步不保证连续交易日。训练窗口W∈{80,120}，至少40样本；训练集sd下限1e−5，距离d²=Σclip((xⱼ−xₜ)/sd,−5,5)²，取k∈{8,16,24}个最近邻。μ=mean(y)，u=sd(y)/sqrt(max(k/5,1))，D=max(0.02,−Q₀.₁(y))，edge=μ−0.002−λu，λ∈{0,0.5,1}。18组参数在过去40步每5步验证、至少4事件；目标mean(reward)−0.5sd(reward)，与持币比较。尚无独立最终测试集，不是完整策略回测。';
const integration = '非防守模型：gate && edge>0 && validation_score>0 → buy_candidate，否则wait。模型内上限=min(0.20,0.80/N,0.005/D)×clip(edge/D,0,1)，账户参考上限=模型内上限×配置权重。跨模型目前分别输出，不净额下单，不自动重分配权重。5步期限、8%收盘风险复核及门槛失效只描述复核政策。执行前仍需账户现金、整手、T+1、涨跌停、流动性和总风险校验。';
const context = '配置→按cutoff读取PIT输入→日期/质量校验→构建池内情绪→逐股特征→成熟标签与历史验证→预测→模型门槛→研究候选/仓位上限→冻结报告。每层携带workspace_id、as_of、input_hash、模型/代码版本；后层不得重抓新行情混入旧上下文。报告含来源、原始输入、18组参数结果和相似日期。缺失返回blocked，不编造0分；回放只读，不写订单。';
const pilot = (gate:string):EngineContract => ({status:'已接入 · 日线试验适配 pilot-gates-1',inputs:bars,features,fit,gate,integration,context});
export const engineContracts: Record<string,EngineContract> = {
 'ak-state-v1':pilot('Bₜ≥0.25。当前只使用近邻状态估计，不声称拟合了完整市场状态模型。'),
 'ak-regime-v1':pilot('Bₜ≥0.50 且 MA20偏离>0。目标模型应在训练集拟合Gaussian HMM（2/3状态候选），前向过滤p(sₜ|x≤ₜ)，按状态条件收益配置；不得用全序列平滑概率回填历史决策。HMM当前未实现，不能把固定门槛称为状态学习。'),
 'ak-continuation-v1':pilot('Cₜ>max(Hₜ₋₂₀:ₜ₋₁) 且相对成交量≥1.2 且 Bₜ≥0.50。突破区间不含当日；结合edge与验证目标才产生候选。'),
 'ak-repair-v1':pilot('60步收盘回撤<−0.08，Bₜ>Bₜ₋₁，且 Cₜ>Cₜ₋₁。只表示恐慌后修复研究门槛，不证明反转成立。'),
 'ak-cooling-v1':{...pilot('Bₜ<Bₜ₋₁ 且MA20偏离>0.05 且相对成交量>1.5。gate为真输出reduce_review，否则wait；不产生新买入候选。'),integration:'只返回已有持仓减仓复核提示，当前研究配置无持仓，仓位上限为0，不代表清仓指令。不创建卖单。未来需把实例position_snapshot引入，再按可卖数量及风控预算生成目标差额。'},
 'ak-event-v1':{status:'设计契约 · 适配器未实现',inputs:[...bars,['event_pit','event_id,published_at,available_at,symbol,text_hash,type','实际公开时间≤cutoff；相同事件去重；需历史预期基线及公司行动']],features:'计划：事件超预期 surprise=(actual−consensus)/历史预测误差sd；异常收益AR=r−β×基准收益，β只用事件前120日拟合；CAR(h)=ΣAR，h=1/3/5/10。文本输出方向、置信度和引用，不能替代预期基线。',fit:'计划：以事件为样本，按公开日期做扩展窗口训练；同事件簇不能跨训练/验证。岭回归预测后5日成本后收益；α由训练内时间切分选择。至少60个独立成熟事件，否则blocked；该最小样本数为待验证设计参数。',gate:'计划：超预期为正、预测成本后收益>0且风险通过才候选；负面事件仅进入风险复核。当前统一blocked。',integration:'缺事件时间、基准或历史一致预期时不退化成技术信号；计划输出event_id、surprise、CAR、prediction和expiry，送同一风险聚合层。',context},
 'ak-quality-v1':{status:'设计契约 · 适配器未实现',inputs:[...bars,['financial_pit','report_period,published_at,revision_id,ROIC,CFO,profit,debt,industry','按实际公开时间连接行情；修订数据保留历史版本'],['valuation_pit','date,market_cap,PE,PB,FCF_yield','亏损PE无效；行业适配，禁止跨行业机械排名']],features:'计划：质量Q=0.45×经营质量+0.35×估值吸引力+0.20×财务安全；子项按行业历史参考分布转0–100分。ROIC、CFO/净利润、增长稳定性、杠杆及估值方向单独定义。缺项不重新归一伪造完整分；需要有版本的行业参考统计输入，不在实例内扫描全市场。',fit:'计划：质量和市场机会分开；以每个财报公开后的观测时点形成样本，60步超额收益标签，成熟后才能训练。初版固定权重作对照，再用非负岭回归验证候选权重，按财报时间分块切分。权重为研究假设，未训练。',gate:'计划：Q≥70、机会edge>0且财务硬风险未触发；70为待验证阈值。当前财报/估值适配不齐返回blocked。',integration:'输出质量、机会、风险三个独立字段及因子贡献，不用高质量抵消流动性或事件硬限制。质量不直接转换成下单概率。',context},
 'ak-ensemble-v1':{status:'设计契约 · 适配器未实现',inputs:[...bars,['child_oos_predictions','model_version,symbol,decision_at,horizon,edge,realized_return,available_at','必须为历史样本外预测；同股票同预测期限对齐，禁止用训练内拟合值'],['instance_risk','capital_weight,cash,positions,risk_budget','仅实例范围读取；研究和交易账本隔离']],features:'计划：子模型成本后edge向量、预测相关矩阵、成熟误差、市场情绪状态。子模型缺失不能用0冒充中性。池内情绪与大盘情绪分别标记scope。',fit:'计划：在成熟样本上求 minₐ Σ(y−aᵀp)²+λ||a−a_prev||²，约束a≥0、Σa≤1，剩余现金。λ在训练内时间切分选择；至少60个对齐成熟样本。训练段估计误差协方差，额外验证成本和权重漂移。当前无优化器实现。',gate:'计划：组合edge>0、验证优于固定等权对照、相关性及风险预算通过；无足够OOS输入则blocked，不能直接平均前5个相关适配器当独立模型。',integration:'用户资金权重是硬上限；学习权重只在已分配预算内调整。先模型组合→按股票净额→组合风险→目标仓位→账户业务校验；发布新参数版本需要独立验证，不能改写历史结果。',context},
};
export const contractTabs = ['数据输入','计算与建模','决策整合','上下文流转'] as const;
