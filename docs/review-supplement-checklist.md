# AlphaK Review 补充清单

审阅日期：2026-10-21。用途：由项目负责人提示 DSH 逐项核对、处理并回填结论。

本清单关注文档一致性、可实现性和验收边界，不将正在开发的未完成功能视为缺陷。未做完整代码审计，也不据此认定 MVP 已完成或未通过测试。本文不是新的产品需求，不授权扩大 MVP 范围；涉及范围或架构的选择应沿用已确认的用户决定，并同步相关文档。

状态约定：`待处理`、`待确认`、`部分处理`、`已解决`、`延期至 V1`。关闭条目时请记录修改文件和验证依据。

## 一、当前 MVP 优先核对

### REV-01 技术方案已补充变更记录，但基线与验收任务仍需同步

- [x] 状态：已解决；优先级：高。
- 依据：`docs/requirements-analysis.md` §0.3、§7.3；OpenSpec `design.md` 技术栈章节及 `tasks.md` 项目骨架、迁移、部署与验收任务。
- 处理前观察：README 和 `AGENTS.md` 已说明当前为 Rust + Axum + SQLite、Vite + React 的演示垂直切片；`design.md` 已增加实现偏差说明；`tasks.md` 的前后端初始化和部分数据库任务也已调整。因此，先前 review 中“没有找到变更记录”的判断已更新。
- 处理前剩余问题：需求冻结基线和设计正文仍写 Next.js + FastAPI + PostgreSQL/TimescaleDB + Redis，任务中仍保留 Compose 等目标架构要求。现已明确当前切片、完整 MVP 目标与 V1 三种状态；后续“恢复目标架构”仅指数据库/基础设施，不恢复 FastAPI。
- 影响：若演示切片、正式目标与当前验收混用，会出现开发按一个方案推进、验收按另一个方案执行的情况。
- 建议：沿用已确认的 Rust 决定，明确哪些是当前切片方案、哪些是后续目标；同步需求、设计、任务与实际可执行的验证命令。不要因本条擅自恢复旧技术栈。
- 关闭条件：当前交付目标、临时替代项、后续迁移项及验收命令有一致且明确的说明。
- DSH 处理记录：已修改 `docs/requirements-analysis.md`（v0.7 的 0.3/7.3 区分当前切片与目标）、`design.md`（交付状态、§2 技术栈、明确不恢复 FastAPI）、`tasks.md`（顶部交付状态、1.4 标记目标架构）、`README.md`、`docs/implementation-status.md`。验证：`.dsh/bin/openspec validate "alphak-platform-mvp" --strict` 通过；任务进度 100/111；当前切片 `cargo test`、`npm run build` 可执行。

### REV-02 容器部署与回环监听约束需区分网络层次

- [x] 状态：已解决（不使用 Docker）；优先级：高。
- 依据：需求 §7.4；OpenSpec `tasks.md` Docker Compose 任务；`specs/platform-foundation/spec.md` 本地访问限制；`design.md` 安全章节。
- 问题：多容器部署要求 web/API/worker 等通信，但规范同时要求服务只能监听 `127.0.0.1`、非回环地址直接拒绝启动。在通常的 Docker bridge 网络下，API 容器内部只监听回环地址会使其他容器及端口映射无法访问。
- 影响：按规范实现的安全限制可能阻断规范要求的部署拓扑。当前宿主机直接运行的演示切片不因此被判定为不可用。
- 建议：明确宿主机发布端口绑定回环地址，与容器内部私有网络监听的区别；约定 API 是否只由 web 代理访问，以及数据库/Redis 是否无需发布宿主机端口。若容器部署延期，应同步当前验收范围。
- 关闭条件：部署拓扑、绑定地址、访问路径与测试要求一致，本地可用且局域网不能直接访问。
- DSH 处理记录：已按用户决定取消 Docker/Compose：`design.md` §16 改为主机回环直跑，`docs/requirements-analysis.md` §7.4、`docs/deployment.md`、`tasks.md` 1.4/11.1 同步；API/Web 直接本机进程运行。不再存在容器内监听与回环约束冲突。

### REV-03 日线同步调度与新鲜度指标冲突

- [x] 状态：已解决（文档口径）；优先级：高。
- 依据：需求成功指标、NFR-06、§6.4；`specs/market-data/spec.md` 数据新鲜度要求。
- 问题：要求在数据源可用的前提下，收盘后两小时内提供日线；A 股正常 15:00 收盘对应最迟 17:00。但同步策略建议 18:00 后才触发。
- 影响：即使同步立即成功，也无法满足时限。
- 建议：统一启动时间、完成期限、重试预算及数据源延迟时的计时/豁免方式，页面保留明确的数据截至时间。
- 关闭条件：调度配置与新鲜度验收可同时满足，源延迟与平台失败可区分。
- DSH 处理记录：已修改 `docs/requirements-analysis.md` §6.4、`specs/market-data/spec.md`（17:00 deadline / 源晚发布）、`design.md` §5、`tasks.md` 4.9。调度代码尚未实现，属完整 MVP 目标；文档口径已可同时满足。

### REV-04 财报覆盖更新不足以支持历史可复现

- [x] 状态：已解决；优先级：高。
- 依据：需求 §6.1、§6.2、point-in-time 与可复现要求；OpenSpec `design.md` 数据模型及 staging/upsert 方案。
- 问题：`financial_report` 以 `(symbol, report_period, statement_type)` 唯一，只记录 `announce_date`，通过 upsert 更新。同一期财报更正或重述时，旧版本可能被覆盖；仅保存公告日或哈希不能恢复被覆盖的内容。
- 影响：后续历史研究、回测或回放无法可靠还原当时可得及实际使用的数据。回测虽在 V1，历史数据保存策略现在就会影响信息是否丢失。
- 建议：明确修订版本、发布时间、采集时间，以及按历史时点选择版本的规则；或者保存可恢复的不可变数据快照。缺乏历史版本的数据源应声明能力边界。
- 关闭条件：用“初次公告 → 更正公告”的样例证明两个历史时点可获得各自正确版本，重复同步不破坏历史。
- DSH 处理记录：已修改 `docs/requirements-analysis.md` §6.1/6.2、`specs/market-data/spec.md`（新增财报版本 requirement + 3 个 scenario）、`design.md` §3/§5、`tasks.md` 2.8/4.16；已实现不可变版本迁移、种子更正版本、`GET /fundamentals?as_of=` point-in-time 查询与集成测试。验证：`cargo test` 通过；历史时点返回 v1，当前返回 v2。

### REV-05 备用数据源的 MVP 范围不一致

- [x] 状态：已解决（范围对齐）；优先级：中。
- 依据：OpenSpec `design.md` 数据适配器与 Open Questions；`specs/market-data/spec.md` 切换数据源场景；需求版本范围表。
- 问题：设计按“AkShare 必做、Tushare 预留接口”推进，但规范要求管理员切换到备用源后仍可正常同步。接口占位不能通过真实切换验收。
- 影响：任务范围小于规范要求，难以确定何时完成验收。
- 建议：明确 MVP 是否实现第二个真实数据源；若仅预留接口，将真实备用源切换场景延期，并保留单源失败时缓存可读、错误可见的当前验收要求。
- 关闭条件：proposal、design、specs、tasks 对数据源数量及切换能力的范围一致。
- DSH 处理记录：已修改 `specs/market-data/spec.md`（MVP 只验收 AkShare，真实备用源切换延期 V1；mock 适配器验证接口）、`design.md` 适配器与 Open Questions、`proposal.md`。当前演示切片仍无真实数据源。

## 二、V1 Agent 契约提前修订，不扩展当前 MVP

### REV-06 决策示例与资金、风险约束不一致

- [x] 状态：已解决；优先级：中；适用阶段：V1 契约。
- 依据：`docs/specs/examples/strategy-instance.example.yaml`、`agent-profile.example.yaml`、`agent-decision.example.json`。
- 问题：实例分配资金 40,000 元，单笔上限 50,000 元，单票权重上限 20%；示例买入茅台 100 × 1,680.5 = 168,050 元、五粮液 200 × 142.3 = 28,460 元，合计 196,510 元。理由却称现金充足、按 12.5% 目标权重建仓。示例没有给出足以解释这些金额与约束的组合上下文。
- 影响：格式合法的示例可能被误当作业务校验成功样例，误导联调与测试。
- 建议：提供上下文、现金、持仓、订单与风险限额一致的成功样例；现有样例可作为明确标注的拒绝案例，列出预期错误码。成功执行样例还应交代 kill switch 的启停状态。
- 关闭条件：示例既通过 Schema 校验，也具有明确的业务校验预期；成功与拒绝案例分清。
- DSH 处理记录：已修改 `docs/specs/examples/strategy-instance.example.yaml`（allocated_cash 1,000,000）、`agent-profile.example.yaml`（max_order_value 200,000）、`agent-decision.example.json`（上下文/风险说明/金额）、新增 `examples/README.md`。验证：Ajv 对 response/decision 示例通过；金额 168,050 + 28,460 = 196,510，占 19.65%，单笔与单票限额均满足。

### REV-07 决策级拒绝与订单级拒绝的语义不统一

- [x] 状态：部分处理（契约已定，测试待 V1）；优先级：中；适用阶段：V1 契约。
- 依据：需求 §0.2 失败关闭、FR-15 校验流程；`docs/specs/README.md` §10 错误码。
- 问题：需求称校验失败不得下单、任一失败则拒绝；错误码表对池外标的拒绝整个决策，对资金不足、T+1、价格和手数错误只拒绝单笔订单。
- 影响：一组调仓订单可能卖出获准、买入被拒，最终组合偏离决策；不同实现可能产生不同结果。
- 建议：明确业务校验是否要求整个决策原子接受，或允许部分订单被拒；规定组合预校验、资金预占、订单顺序、人工审批后再次校验及幂等规则。区分“校验接受”与后续逐笔撮合成交，不承诺无法保证的成交原子性。
- 关闭条件：混合合法/非法订单、卖出后买入、审批期间现金变化等案例有一致且可测试的结果。
- DSH 处理记录：已修改 `docs/specs/README.md`（校验按决策原子化、审批后重校验、校验 vs 撮合）与 `docs/requirements-analysis.md` FR-15；错误码表改为拒绝整个决策。实现属 V1，本次只改契约。

### REV-08 模型响应与平台完整决策缺少独立契约

- [x] 状态：部分处理（契约已定，测试待 V1）；优先级：中；适用阶段：V1 契约。
- 依据：`docs/specs/README.md` §8；`agent-profile.example.yaml` 的 system prompt 和 output_contract；`agent-decision.schema.json`。
- 问题：执行协议要求模型只返回语义字段，运行时注入 decision_id、实例/Agent 标识、版本、as_of、context_hash；示例提示词及 output_contract 却要求模型严格返回完整决策 Schema。
- 影响：结构化输出约束可能强迫模型编造运行时字段；不同接入实现会对字段归属作出不同解释。
- 建议：定义独立的模型响应 Schema；运行时验证语义响应、注入可信 envelope，再验证完整决策并执行业务校验。同步提示词、模板与示例。
- 关闭条件：模型输出与平台生成字段边界明确，模型伪造 envelope 的处理方式有测试案例。
- DSH 处理记录：新增 `docs/specs/agent-response.schema.json` 与 example；更新 `agent-profile.schema.json`、`agent-profile.example.yaml`、`docs/specs/README.md`、`index.json`、`AGENTS.md`、`docs/requirements-analysis.md`。验证：Ajv 对 response 与 decision 示例均通过；profile 示例通过 schema。

## 三、文档维护

### REV-09 清理过期背景与已冻结问题

- [x] 状态：已解决；优先级：低。
- 依据：需求 §0.1、§0.3、§13；OpenSpec `design.md` Context。
- 问题：前提假设仍称市场、规模与技术栈未确认，而后文已有冻结基线；设计 Context 仍称没有应用代码。开放问题中已经决策的条目应与最新基线核对。
- 建议：将历史背景明确标为规划时状态；将已决策问题标记结论及依据，保留真正待确认的问题。区分当前演示切片、MVP 目标与 V1 能力。
- 关闭条件：新加入的开发 Agent 能从文档直接识别当前状态、已确认决策和待办范围，无需猜测哪段文字优先。
- DSH 处理记录：已修改 `docs/requirements-analysis.md` §0.1/§0.3/§13（规划时假设、冻结结论、Q8）、`design.md` Context（当前已有演示切片）、`proposal.md`（规划时 vs 当前）。验证：OpenSpec validate 通过；无残留“没有应用代码/Next.js+FastAPI”主张（除明确的历史/否决说明）。

## 建议交接顺序

1. 先处理 REV-01，统一当前交付基准和验收命令。
2. 对齐 REV-02、REV-03、REV-05 的部署、调度与范围决策。
3. 在正式接入财务数据前明确 REV-04 的版本保存方式。
4. 将 REV-06～REV-08 纳入 V1 契约修订，不提前实现 Agent 功能。
5. 同步处理 REV-09；逐项填写结论后，再决定是否关闭条目。

## 四、实现复审（2026-10-21）

本轮扩展至当前演示切片代码，以下为新增实现问题；不将真实数据源、生产部署、V1 Agent 等已披露待办列为新缺陷。REV-01～09 的 DSH 处理记录保留。本轮未修改业务代码。P1 表示优先修复的数据展示正确性问题，P2 表示常规功能缺陷。请求竞态与图表问题基于代码路径审查，尚未做浏览器延迟注入或交互复现。

### REV-10 个股请求缺少过期响应隔离，可能串标的或串周期

- [x] 状态：已解决；优先级：P1。
- 依据：`apps/web/src/pages/Stock.tsx:31`、`:46` 两个 effect。
- 触发：从 A 快速切换到 B，或快速切换日/周/月线；让先发出的请求后返回。
- 问题：stock/fundamentals/profile/bars 均直接提交响应，没有 cleanup、请求序号或当前参数检查。旧请求可覆盖当前页面；行情标题、图表和财务数据可能来自不同标的。旧请求失败也可将已成功的新页面覆盖为错误页。
- 建议：按 symbol/period/adjust 隔离请求，忽略过期结果与错误，必要时取消请求；切换标的时清理对应展示状态。
- 关闭条件：通过可控的反序响应验证，最终数据均对应当前 URL 与控件参数，旧失败不影响新成功结果。
- DSH 处理记录：已修改 `Stock.tsx`：stock/fundamentals/profile/bars 四个请求均用 effect 局部 `active` 标记隔离过期响应；切换标的时清空详情/财务/资料；新增 Playwright 反序响应用例（延迟 600519 后切到 000858，等待旧响应后仍显示五粮液）。

### REV-11 CSV 导出再导入会污染备注，特殊字符无法往返

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/web/src/pages/Watchlist.tsx:95` 的 exportCsv 与 importCsv。
- 复现：将代码 `600519.SH`、名称“贵州茅台”、备注“长期,关注”导出，再导入一个空分组。现有算法得到备注“贵州茅台 长期 关注”，而不是原备注。
- 问题：导出是 symbol,name,note 三列，导入却把第一列之后的所有列拼成备注；导出直接将逗号替换为空格，导入按逗号/换行拆分，不支持 CSV 引号与换行。上述往返算法已独立执行验证。
- 建议：统一列定义，按表头取值，正确编码和解析引号、逗号、换行；重复项不要无条件计为新增成功，失败应提供行号和原因。
- 关闭条件：空备注、中文、逗号、双引号、换行，以及导出文件导入新分组均保留字段原值；重复项有明确统计语义。
- DSH 处理记录：新增 `apps/web/src/csv.ts`：RFC 风格引号/逗号/换行转义与解析；导出固定 `symbol,name,note` 表头；导入按表头取值，重复项跳过并统计，失败带行号/代码；新增导出→新建分组→导入 E2E，备注含逗号、双引号、换行仍保持原值。

### REV-12 默认首页与列表密度保存后不生效

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/web/src/pages/Settings.tsx:77`、`:85`；`apps/web/src/App.tsx`；`apps/web/src/components/Shell.tsx`。
- 问题：这两个字段有设置控件和后端持久化，但前端没有消费值的路径。Shell 加载设置只应用 up_color，根路由始终渲染 Dashboard，也没有按 density 应用样式。
- 复现：保存默认首页为自选、密度为紧凑，然后重新打开根地址；仍显示市场首页，列表密度不变。此结论来自全源码引用检查，未执行浏览器操作。
- 建议：接入设置实际行为，启动时按默认首页导航，列表使用密度样式；若本切片延期，应禁用控件并说明尚不生效，同步实现状态。
- 关闭条件：保存后与重新加载后行为一致；直接打开个股深链接不会被默认首页设置错误覆盖。
- DSH 处理记录：`Shell.tsx` 在首次加载时应用 `default_home`（仅根路径，不覆盖个股深链）与 `density`；`Settings.tsx` 保存后立即应用密度；新增 `data-density` 样式与 Playwright 用例（默认自选、紧凑、深链不被覆盖）。

### REV-13 K 线主图与指标副图时间范围未同步

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/web/src/components/KLineChart.tsx:58`、`:62`、`:86`。
- 问题：主副图是两个独立 chart，各自 fitContent；没有可见时间范围同步。主图缩放或拖动后，MACD/KDJ 仍保持自身范围，对应横向位置不再代表相同交易日。副图还过滤 warmup 空值，可能从不同起始日期 fitContent。
- 建议：共享时间轴或同步可见范围，并处理指标 warmup 的时间占位与两侧价格轴宽度。
- 关闭条件：缩放、拖动、切换指标与调整窗口后，主副图相同日期始终对齐。
- DSH 处理记录：`KLineChart.tsx` 为主副图订阅 `VisibleTimeRangeChange` 并双向同步，带重入保护；初始范围对齐；容器暴露 `data-visible-range` 供回归测试；Playwright 拖动主图后断言主副图范围一致。

### REV-14 K 线错误页的重试按钮不发请求

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/web/src/pages/Stock.tsx` 的 `onRetry={() => setPeriod(period)}`。
- 触发：个股档案成功、bars 请求失败，然后点击图表的重试按钮。
- 问题：把 period 设置为当前值不会触发依赖 symbol/period/adjust 的 effect，因此不会清除错误或重新请求。用户只能切换周期等方式间接恢复。
- 建议：显式复用加载函数，或使用独立重试计数触发请求；重试仍需遵守 REV-10 的过期响应隔离。
- 关闭条件：同一周期失败后恢复接口，点击一次重试产生新请求并恢复图表。
- DSH 处理记录：`Stock.tsx` 新增 `barReload` 计数并加入 bars effect 依赖；重试按钮递增计数而非重复设置 period；Playwright 让 StrictMode 的前两次 bars 请求失败，点击重试后新请求恢复图表。

### REV-15 README 的 .env 配置步骤不会被后端加载

- [x] 状态：已解决；优先级：P2。
- 依据：`README.md:29`；`apps/api/src/main.rs:22`、`:29`；`apps/api/Cargo.toml`。
- 问题：README 引导复制 `.env.example` 到 `.env`，但后端只读取进程环境变量，没有 dotenv 加载或启动脚本导出步骤。用户只修改 `.env` 后按文档启动，绑定地址与数据库路径仍为默认值，可能操作错误的数据库。
- 建议：明确提供环境变量导出/启动命令，或在启动时加载 .env；解释数据库相对路径的基准目录。
- 关闭条件：在未预先 export 变量的终端按 README 配置临时数据库与端口，实际监听和文件位置与配置一致。
- DSH 处理记录：`main.rs` 接入 `dotenvy::dotenv()`；README 说明 `.env` 从当前工作目录加载、`ALPHAK_DB` 相对路径基准；用临时目录 `.env` 验证实际监听 8099 且数据库文件落在配置路径。

本轮最终验证：`cargo test --manifest-path apps/api/Cargo.toml` 62 个测试全部通过；`cargo clippy -- -D warnings` 通过；`npm run build` 通过；`npx playwright test` 10 个用例全部通过；OpenSpec `validate --strict` 通过。前端仍有单个 JS chunk 超过 500 kB 的警告。

## 五、后端数据与恢复复审（2026-10-21，后续轮次）

当前进度文档为 73/102。复审时在临时目录、临时 SQLite 和独立回环端口复现了以下 5 项；本轮已修复并回填验证。恢复脚本测试使用临时文件，未覆盖项目数据库。DSH 已回填 REV-10～15 的修复与验证记录，本轮保留，不重复登记旧问题。

### REV-16 复权函数未接入 bars 接口，三种模式实际返回原始价格

- [x] 状态：已解决；优先级：P1。
- 依据：`apps/api/src/routes.rs:345` bars；`apps/api/src/adapter.rs` adjust_bars。
- 复现：在隔离数据库将 600519.SH 最后一天 adj_factor 设为 2，其余为 1，分别请求 bars?adjust=none/qfq/hfq。三种模式首尾收盘价均为 `[1747.2886342803781, 1485.566907218708]`。
- 问题：API 查询不读取 adj_factor，也不调用已有 adjust_bars，仅将请求模式回填到响应。单独的复权函数测试通过不能证明接口生效，默认种子因子全为 1 会掩盖问题。
- 建议：读取因子，先完成价格复权再聚合周/月线并计算指标；验证 period/adjust 枚举。
- 关闭条件：通过实际 bars 路由验证除权前后数据，qfq 保持最新价格、hfq 按约定锚点转换，周/月 OHLC 与指标由相应复权价格产生。
- DSH 处理记录：已修改 `bars` 接口：读取 `adj_factor`，按 none/qfq/hfq 先复权再聚合周/月线并计算指标；校验 period/adjust 枚举。新增路由级测试：最后一天 factor=2 时，qfq 最新价不变、首日减半，hfq 首日不变、最新翻倍。

### REV-17 财务历史查询仅过滤报表，指标与估值仍泄露未来数据

- [x] 状态：已解决；优先级：P1。
- 依据：`apps/api/src/routes.rs:428`、`:442`、`:446`；`apps/api/src/db.rs` financial_indicator 主键；`apps/api/src/seed.rs` 财报修订。
- 复现一：请求 fundamentals?as_of=2020-01-01，reports 为 0 条，但 indicators 为 5 条，valuation.as_of 为 2026-10-21，返回查询时点之后的数据。
- 复现二：当前 600519.SH 的 2026-06-30 修订版净利率按 net_profit/revenue 计算为 8.07920792079208%，同一响应的 financial_indicator.net_margin 仍为 8%。修订未同步到衍生指标；单版本指标表也无法对应历史报表版本。
- 建议：指标绑定所选报表版本或从该版本重新计算；估值日期及分位样本按 as_of 截断。若 as_of 仅承诺报表过滤，应拆分接口或明确字段时点边界，不把整个响应当作历史快照。
- 关闭条件：早于所有公告的查询不返回未来指标；修订前后指标各自与报表一致；估值及分位只使用对应时点可用的数据。测试断言字段日期和值，不能仅搜索 version 字符串。
- DSH 处理记录：已修改 `fundamentals`：`as_of` 同时约束报表、指标与估值样本；指标由所选报表版本重新计算净利率/负债率/ROE，EPS/BPS 使用对应时点股本；早于公告的查询 reports/indicators 为空、valuation.as_of 为空。测试断言 2026-06-30 v2 的 net_margin 与报表一致。毛利率仍取源数据（报表 payload 无成本字段）。

### REV-18 涨跌停统计统一使用 9.8%，创业板普通上涨被计为涨停

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/api/src/market.rs:122`、`:125`。
- 复现：隔离库将创业板 300750.SZ 最新收盘设为前收的 1.10 倍，刷新 overview；个股涨幅约 10%，breadth.limit_up 返回 1。该股票通常适用 20% 限制，10% 不应计为涨停。
- 问题：所有证券统一采用 ±9.8% 阈值，没有按板块、ST、上市阶段等规则判定，也没有按最小价格单位核对实际涨跌停价；主板接近涨停但未到涨停价也会误报。
- 建议：使用可信涨跌停价/标记，或按证券规则和日期计算；缺少数据时显示不可得，避免以近似阈值冒充准确家数。
- 关闭条件：覆盖主板、创业板/科创板、ST 和无涨跌幅限制情形，包含接近涨停但未封板的反例。
- DSH 处理记录：已修改 `market.rs`：涨跌停按主板 10%、创业板/科创板 20%、北交所 30%、主板 ST 5% 计算，并按 0.01 元价格取整比较；测试覆盖创业板 10% 不计涨停、20% 计涨停。

### REV-19 52 周高低价实际计算全历史极值

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/api/src/routes.rs:249`。
- 复现：在隔离库加入 600519.SH 的 2020-01-02 行情，high=99999。最新行情仍截至 2026-10-21，但个股接口 high_52w 返回 99999。
- 问题：MAX(high)/MIN(low) 仅按 symbol 过滤，没有 52 周窗口；当前约一年种子数据掩盖问题，累积或补入历史行情后页面口径错误。
- 建议：以该股票最新行情日期为锚点限制 52 周范围，明确不足一年、停牌及复权口径。
- 关闭条件：窗口外极值不影响结果，边界日期与无近期行情情形有明确测试。
- DSH 处理记录：已修改 `stock_detail`：52 周高低以最新行情日为锚点限制 `date(latest, '-1 year')` 窗口；测试插入 2020 年 high=99999 后接口不再返回该极值。

### REV-20 恢复只验证 SQLite 文件结构，非 AlphaK 数据库也会覆盖当前库

- [x] 状态：已解决；优先级：P2。
- 依据：`scripts/restore.sh:11`、`:27`；README 备份恢复流程。
- 复现：临时目录创建只有 unrelated 表的 SQLite 文件且不提供 .sha256，目标库含 watchlist_item 和一条备注。执行恢复返回 exit 0，目标库仅剩 unrelated 表，原 watchlist_item 不再存在。恢复前安全备份存在，可以手工恢复，但错误源文件已被报告为恢复成功。
- 问题：checksum 文件不存在时静默跳过校验，integrity_check 只能证明 SQLite 物理结构正常，不能确认 AlphaK schema、版本和必要表；恢复后没有应用层检查。
- 建议：恢复前验证备份来源/清单、应用 schema 及版本兼容性，显式处理缺失校验信息；恢复后运行应用层检查，失败时提供安全回滚流程。
- 关闭条件：无关 SQLite、缺失校验清单、不可兼容 schema 按明确规则拒绝且不覆盖目标；合法备份恢复后设置和自选可读，并验证自动迁移兼容性。
- DSH 处理记录：已加固 `scripts/restore.sh`：强制要求 `.sha256`，恢复前校验 checksum、SQLite integrity、必需表与 `financial_report.version`；恢复后做同样应用层检查，失败时自动回滚到 pre-restore 备份。实测无关 SQLite 与缺失 checksum 均 exit 2 且不覆盖目标。

## 六、暂存、质量与输入契约复审（2026-10-21，后续轮次）

此轮基于进度文档 78/102 与新增暂存模块审查。复审时 `cargo test` 31/31 通过；以下 5 项已在后续实现中修复，暂存函数也已接入同步/补数路径。当前进度 100/111，Rust 测试 62/62 通过。REV-16～20 已有 DSH 修复记录，本轮不重复登记。

### REV-21 暂存重传只更新校验状态，可能把旧的非法价格晋升为正式行情

- [x] 状态：已解决；优先级：P1。
- 依据：`apps/api/src/ingest.rs:50`～`:52` 的 ON CONFLICT；`:61`～`:64` 的 promote_valid_quotes。
- 复现：同一 symbol/date 首次暂存 close=1500、high=1490，判定 rejected；随后重传修正数据 close=1500、high=1510，判定 valid。执行现有 SQL 后，暂存实际保留 high=1490、close=1500，但 status 已为 valid；晋升后正式行情仍是 high=1490、close=1500。
- 问题：冲突更新只替换 status/reason/ingested_at，不替换用于校验的 OHLC、成交量和因子。校验对象与落库对象不同，导致质量闸门反向放行旧坏数据；即使先后两次都合法，也会丢失价格修正。
- 建议：同一 key 的完整载荷与校验状态原子更新，或使用不可变批次版本；晋升只能采用与校验结果绑定的载荷。
- 关闭条件：通过 stage_quotes/promote_valid_quotes 函数测试 rejected→valid、valid→valid 修订、valid→rejected，确认正式库只包含实际通过校验的最新载荷。
- DSH 处理记录：已修改 `stage_quotes` 冲突更新：OHLC、成交量、成交额、复权因子与校验状态原子替换；新增 rejected→valid、valid→valid 修订、valid→rejected 三个测试，晋升库只包含最新通过载荷。

### REV-22 质量报告用已有行情推算应有日期，整天行情缺失仍显示满覆盖

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/api/src/sync.rs:170`、`:173`、`:185`，recompute_quality。
- 复现：临时数据库初始 quote_daily 覆盖率 1.0、missing=0；删除最新交易日 2026-10-21 的全部 13 条行情后重新请求 admin/quality，仍返回 coverage=1.0、missing=0、anomalies=0。
- 问题：dates 来自 quote_daily 的 COUNT(DISTINCT trade_date)，整天缺失使实际行数和预期行数一起减少，缺失被掩盖。daily_basic/index_quote 的预期数量也依赖这张可能不完整的行情表，不能独立判断各数据集完整性。
- 建议：按声明的覆盖区间、交易日历、证券上市/退市及停牌规则建立预期集合，再与各数据集独立对比；不要把待检查表本身当作完整日期基准。
- 关闭条件：整天缺失、单标的缺失、其他数据集日期与行情不同步均有明确缺失报告；新上市与停牌不被错误算成缺失。
- DSH 处理记录：质量报告改为用 `trading_calendar` 的 open 日与 `instruments` 上市/状态计算预期数量；新增整天缺失测试，coverage<1、missing>0。

### REV-23 指数成员只检查退出时间，未来才加入的成员提前出现

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/api/src/routes.rs:692` index_member_rows；sector_member_rows 也未检查 in_date。
- 复现：将 000001.SH 的现有成员 in_date 全部设为 2099-01-01，当前指数详情仍返回 10 个成员，尽管详情行情日期在 2026 年。
- 问题：指数查询仅判断 out_date，未要求 in_date <= 当前查询日期；板块查询仅判断 out_date IS NULL。提前加入的成分可能影响展示、权重和板块聚合。
- 建议：以明确的行情/查询日期过滤有效成员区间，统一 in_date 与 out_date 边界；各聚合入口复用同一规则。
- 关闭条件：未来加入、已退出、边界日及同一证券多段成员历史均有路由测试，当前成员集合与对应时点一致。
- DSH 处理记录：`index_member` 查询加入 `in_date <= 查询日期`；`sector_member` 统一按 `in_date/out_date` 过滤，`market` 聚合也复用；新增未来成员不返回测试。

### REV-24 Axum 参数提取失败绕过统一错误响应契约

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/api/src/routes.rs` 直接使用 Json/Query/Path 提取；`apps/api/src/error.rs` ApiResponse；`apps/web/src/api.ts` request。
- 复现：POST watchlists/groups 请求体为 `{`，GET sectors/not-a-number，以及 GET market/overview?refresh=wrong，均返回 HTTP 400 + text/plain；没有 code/message/data/request_id。
- 影响：前端将这些可解释的输入错误统一显示为“服务返回了无法解析的响应”，生成客户端和请求追踪也无法按约定处理；现有错误契约测试只覆盖已进入 handler 的错误。
- 建议：对 JSON、查询和路径提取拒绝统一转换为 AppError/ApiResponse，并覆盖类型不符、缺字段、非法 JSON 等情况。
- 关闭条件：常见 4xx 提取失败返回统一 JSON envelope 与非空 request_id，客户端显示具体可读原因；补充真实 HTTP/路由契约断言。
- DSH 处理记录：新增 `ApiJson`/`ApiQuery`/`ApiPath` 提取器，将 deserialize/类型错误转为统一 `AppError` 包络；测试非法 JSON、非法 path、非法 query 均返回 JSON code=1 + request_id。

### REV-25 设置接口不校验值域，会永久保存无效首页与选项

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/api/src/routes.rs:878` update_settings；`apps/web/src/components/Shell.tsx` 默认首页导航。
- 复现：PUT settings 提交 default_home=/does-not-exist、poll_interval=-1、up_color=invalid、density=invalid，接口返回 200；随后 GET settings 原样返回这些非法值。
- 影响：浏览器根路径启动会根据持久化的 default_home 导航到不存在的页面，设置控件无法匹配非法选项；UI 的 select 限制无法替代 API 校验。
- 建议：后端验证支持的枚举、默认首页允许路径、时区与轮询区间；批量设置先完整校验再原子保存，非法请求不应部分覆盖合法设置。
- 关闭条件：对非法枚举/路径/负数返回统一 4xx，原设置保持不变；合法值保存及重启应用正常。
- DSH 处理记录：`update_settings` 先合并当前值并校验 theme/up_color/default_home/timezone/density/poll_interval，再事务写入；非法请求返回 4xx 且原设置不变。

## 七、同步范围、版本与重复执行复审（2026-10-21，后续轮次）

本轮基于 100/111 状态审查，独立运行 Rust 测试 62/62 通过，后端构建通过。REV-21～25 的 DSH 修复记录保留，不重复登记。以下问题均在临时数据库、独立回环端口的真实 HTTP 接口验证；REV-26 的前置暂存遗留与 REV-27 的版本到达顺序由临时库构造，具体条件如下。所有实例结束后销毁，未修改业务代码或项目数据库。

### REV-26 晋升所有 valid 暂存记录，补数会越过当前日期和批次范围

- [x] 状态：已解决；优先级：P1。
- 依据：`apps/api/src/ingest.rs:63` promote_valid_quotes；`:204` sync_bars；sync_quote_incremental。
- 复现：在临时暂存区准备一条 600519.SH、2026-10-19、close=100 的合法遗留记录；调用 POST admin/backfill，仅请求 quote_daily 的 2026-10-21～2026-10-21。响应 changed=2，范围外 10 月 19 日收盘价从 1494.1760004882965 变为 100。
- 问题：晋升查询只按 status='valid' 过滤，不按当前请求的 symbol/date/batch；成功后也删除所有 valid 暂存。sync_bars 在混合合法/非法批次失败时会留下合法行，因此后续无关请求可能提交前一次失败批次的数据。补数审计只记录当前日期范围，无法准确解释实际改动。
- 建议：为暂存添加批次身份并绑定晋升，或在同一事务中完成当前批次的 staging/validation/promotion；晋升与清理仅作用于当前请求范围，失败批次有明确保留或回滚策略。
- 关闭条件：构造含合法/非法行的失败批次，再运行不同标的/日期补数，正式库与审计均不受旧暂存污染；范围外数据保持不变。
- DSH 处理记录：`quote_staging` 增加 `batch_id`；`stage_quotes`/`promote_valid_quotes`/`discard_batch` 均按批次操作。补数前构造 10-19 合法遗留暂存，执行 10-20～21 补数后 range 外仍保持 999，测试通过。

### REV-27 旧版财报晚到会形成两个 is_current，当前接口返回同报告期两版

- [x] 状态：已解决；优先级：P1。
- 依据：`apps/api/src/ingest.rs:225` sync_financials；financial_report 当前版本查询。
- 复现：临时库保留 600519.SH、2026-06-30 的 v2 且 is_current=1，删除 v1 模拟先收到修订版；调用 financial_report 补数，公告日期范围 2026-08-01～2026-08-31，使 mock 的 v1 晚到。结果 v1 和 v2 都为 is_current=1，fundamentals 当前接口同一报告期返回版本 `[2,1]`。
- 问题：所有新插入版本都写 is_current=1，只把低于本次版本的旧记录置为 0；当更高版本已经存在时，不会把晚到低版本置为非当前。重复相同版本的 INSERT OR IGNORE 也未验证载荷冲突。
- 建议：事务内按完整报告主键组重算唯一当前版本，保证任意到达顺序一致；为当前版本唯一性增加数据库约束，并明确相同版本不同载荷的拒绝策略。
- 关闭条件：v2→v1、v1→v2、重复版本及同批乱序导入后都只有一个正确的当前版本，历史版本仍可按时点查询。
- DSH 处理记录：`sync_financials` 改为事务内插入后按完整主键重算唯一 `is_current`，增加部分唯一索引；晚到 v1、v2 当前、重复同版本同载荷、同版本不同载荷测试通过（后者拒绝）。

### REV-28 证券主数据同步将拼音索引覆盖为代码，已可用搜索退化

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/api/src/ingest.rs:176` sync_instruments，`:185` 使用 item.symbol.to_lowercase() 写 pinyin。
- 复现：初始 GET instruments/search?q=gzmt 返回 600519.SH；POST admin/data-jobs 触发 instruments 同步后，同一搜索返回空，数据库 pinyin 已变成 600519.sh。名称和代码搜索仍可用，故普通同步成功测试不会发现拼音退化。
- 建议：在适配器 DTO 中携带有效拼音或从名称生成；更新主数据时保留已有有效索引，不用证券代码伪装拼音。
- 关闭条件：同步前后 gzmt/wly 等拼音检索一致；新增证券也有有效检索索引。
- DSH 处理记录：`InstrumentDto` 增加可选 `pinyin`，mock fixture 填 `gzmt/wly`；同步 upsert 不再用代码覆盖拼音，`gzmt` 搜索测试通过。

### REV-29 公司资料同步越权改写每日指标和指数行情

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/api/src/sync.rs:225` 将 daily_basic/index_quote/company_profile 统一调用 ingest::sync_reference；`apps/api/src/ingest.rs:264`。
- 复现：只触发 company_profile 同步，600519.SH 的 2026-10-21 total_mv 从 1865872035466.697 变为 1890280000000；000300.SH 当日 close 从 3793.3916113134296 变为 3793。没有触发 daily_basic 或 index_quote 同步。
- 问题：每个入口都导入整个 reference fixture，实际写入范围不匹配任务数据集与检查点。已成功的“公司资料”任务会隐藏其他数据变化，还可能在其他数据源失败时发布不相关数据。
- 建议：按声明的数据集拆分导入和事务边界；若确实是组合任务，应在 API、任务记录、审计与失败语义中明确列出全部受影响的数据集。
- 关闭条件：单数据集同步不改写无关表；组合任务如被采用，改动清单、状态和检查点可完整追溯。
- DSH 处理记录：`sync_reference_dataset` 按 dataset 过滤；company_profile 同步不再改 daily_basic，daily_basic 同步单独生效，隔离测试通过。

### REV-30 按日期去重所有成功任务，阻止同一天重新同步修复数据

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/api/src/sync.rs:75`、`:86`，idempotency_key=dataset:UTC日期。
- 复现：先成功执行 quote_daily 同步；随后在临时库删除 mock 源可提供的 600519.SH、2026-10-21 行情，再次通过同一接口触发同步。接口返回旧的 succeeded 任务，缺失行仍为 0 条，没有实际运行修复。
- 问题：把“同一请求重试去重”扩大为“同一数据集一天只能成功执行一次”。同日源数据更正、补入数据或当前库变化后，手动同步均无法再执行；成功但变更 0 的早期任务也会挡住之后的定时任务。
- 建议：区分运行请求身份和数据写入幂等，允许新的手动请求/调度窗口启动新任务，只对同一运行的重试和并发重复去重；可提供明确 force 或 run_id 语义。
- 关闭条件：同一请求重试不重复写入，但同日新请求可处理新增/修订/缺失数据；手动与两个调度时段之间不相互误阻塞。
- DSH 处理记录：`RunJobBody` 增加可选 `request_id`，idempotency key 改为 dataset+日期+request_id；调度使用窗口 key。同日先成功、删除行情、再以新 request_id 同步可修复，测试通过。


## 八、免费多源接入与自动补齐复审（2026-10-22）

本轮检查新增 `sources/`、真实同步路由与自动重试。原项目独立运行 `cargo test`：62/62 通过。以下区分隔离副本函数验证和代码路径审查；没有修改业务代码或项目数据库，也没有向 DSH 发送消息。此时实施状态仍将多源任务列为未完成，本清单用于开发中修正已有实现，不将未实现的功能重复计为缺陷。

### REV-31 腾讯日线把缺失成交额写成零，并覆盖已有真实值

- [x] 状态：已解决；优先级：P1。
- 依据：`apps/api/src/sources/tencent.rs:83`；`apps/api/src/sources/store.rs:70`。指数发布也直接覆盖 amount。
- 验证：隔离副本用腾讯格式的六字段日线响应调用实际 `parse_daily`，再调用 `publish_daily` 更新已有 600519.SH、2026-10-21 行情；原成交额大于零，发布后 amount=0。这是缺字段的占位值，不是源端提供的零成交。
- 影响：腾讯是默认主源，每次同步都能清空已有成交额，个股/指数成交额与市场聚合失真；价格校验允许 amount=0，因此批次仍可判成功。
- 建议：缺失字段显式为未知；通过能提供成交额的同口径来源补齐，或按字段来源与有效性保留已有可信值，不能用零表达缺失。
- 关闭条件：六字段腾讯响应不会把已有金额覆盖为零；新行缺金额在 API/页面显示不可得；明确真实零成交与缺失值的差异，并覆盖指数路径。

- DSH 处理记录：新增 `amount_known`；腾讯六字段日线标记未知，发布时保留已有可信金额，新行 API 显示不可得；增加未知金额不清零测试。

### REV-32 自动重试忽略发布错误，写入失败仍标记 fresh

- [x] 状态：已解决；优先级：P1。
- 依据：`apps/api/src/sources/retry.rs:68`～`:101`、`:112`。
- 代码路径：两个分支均在 fetch 成功后的 map 中执行 `let _ = state.db.lock().map(...publish...)`，忽略锁与发布的 Result，并无条件返回来源字符串。外层因此增加 successes，最后写入 fresh。
- 触发条件：源端请求成功，但发布事务由于约束、触发器或数据库写入故障失败；即使只让 quote_daily 的写入失败、data_freshness 仍可写，也会被标记成功。不是对上游故障的推测，而是现有 Result 丢弃路径。
- 建议：把抓取、质量校验、事务提交串为同一 Result；只有 commit 成功才能计成功，发布失败保留待重试任务和错误原因。
- 关闭条件：临时库用拒绝 quote_daily/index_quote 写入的触发器注入故障，抓取成功后仍返回失败/partial，fresh 不被更新，移除故障后可继续补齐。

- DSH 处理记录：`retry_task` 将抓取与发布串成 Result，发布失败重新入队且不更新 fresh；`sync_daily_symbol` 的发布错误也会返回。

### REV-33 自动重试丢失失败任务范围，更新其他股票后清除待补齐状态

- [x] 状态：已解决；优先级：P1。
- 依据：`apps/api/src/sources/retry.rs:8`、`:34`、`:118`、`:138`～`:144`；`routes.rs` 的 source_sync 只保存数据集级 freshness。
- 代码路径：待重试状态只保存 dataset；下轮重新取自选前 5 只/默认股票和最近 10 个自然日，完全不读取失败请求的 symbols/start/end，随后只要这些新请求成功便将整个 dataset 标为 fresh。
- 具体场景：请求一只不在默认自选前 5 的股票、2024 年历史补数失败；自动重试更新当前默认股票最近 10 天后，原股票及 2024 年缺口没有被处理，dataset 却变 fresh，后续不再被 stale_datasets 选中。snapshot 的 stale/partial 也被调度分支直接跳过。
- 建议：持久化待补齐的股票、数据集、日期区间和剩余缺口；重试原任务并按真实进度关闭，汇总状态从未完成任务推导；snapshot 纳入相应重试执行器或明确其手动重试语义。
- 关闭条件：非默认股票、超过 10 天的历史区间、超过 5 只股票以及 snapshot 失败后，都有针对原范围的补齐；无关任务成功不能清除原缺口。

- DSH 处理记录：新增 `source_retry_task(dataset,symbol,start_date,end_date)`；失败/缺口按原范围入队，自动重试重放原范围，snapshot 也纳入。

### REV-34 缺失复权因子仍以 1 入库，图表直接当有效因子使用

- [x] 状态：已解决；优先级：P1。
- 依据：`apps/api/src/sources/store.rs:51`；`apps/api/src/routes.rs:472`～`:506`；`docs/data-sources.md` 声明失败时不伪造因子。
- 验证：隔离副本发布一条新日期行情，adj_factor=None、adj_error=qfq unavailable；实际落库 adj_factor=1、adj_source=""，返回 adj_pending=1。但 bars 路由不检查 adj_source/pending，qfq/hfq 均直接用该 1 参与计算。
- 影响：源端复权抓取失败时，页面仍能生成看似正常的复权图和技术指标；已有行也可能保留原 demo 因子，与新真实价格混用。仅在同步响应增加 pending 计数不能阻止后续错误使用。
- 建议：缺失/演示因子与可信因子显式分离；复权读取路径校验因子完整性与来源，缺失时返回可识别不可用状态或明确降级为不复权并展示说明；后续独立重试补因子。
- 关闭条件：新行缺因子、旧 demo 因子及部分日期因子缺失均不会被当作有效 qfq/hfq；补齐后才恢复复权指标与图表。

- DSH 处理记录：bars 路由校验 `adj_source/adj_batch_id`；缺失时降级 `none`、`adjust_available=false` 并返回说明，新增降级测试。

### REV-35 主源返回不完整数据时，补缺仍优先同一主源，且备用补入被错误标注来源

- [x] 状态：已解决；优先级：P2。
- 依据：`apps/api/src/sources/mod.rs:269`～`:280`；`apps/api/src/routes.rs:1439`～`:1452`（index_quote 同类路径）；`merge_bars`。
- 代码路径：fetch_daily 对任意非空结果立即返回成功；发现日期缺口后再次调用相同优先级的 fetch_daily。若主源在缺口区间仍只返回部分数据，就再次提前返回，备用源永远不尝试，尽管它可能提供完整缺口。
- 另一个可达路径：补缺时主源真正报错、备用成功，merge_bars 只合并 extra.data，丢弃 extra.source/adj_source；随后以原 batch.source 把备用数据全部写成主源来源，破坏追溯。
- 建议：让备用策略识别预期覆盖集合，针对未满足的日期显式尝试下一来源；合并保留逐行行情与因子来源，或按来源分别发布。
- 关闭条件：主源缺两天但返回其中一天、备用提供另一日的可控测试能够补全；备用写入行及复权因子的 source/batch 可准确查询，不冒用主源身份。
- DSH 处理记录：补缺先发布主源已有行，再按未满足日期显式调用下一来源 `fetch_daily_from`，每批独立 source/batch；备用行不冒用主源。
