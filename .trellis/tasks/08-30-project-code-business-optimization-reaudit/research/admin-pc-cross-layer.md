# Research: Admin React 与 PC Vue/Tauri 跨层复审

- Query: 复核当前 Admin React、PC Vue/Tauri 及其 Rust 后端契约，覆盖 API/DTO、权限、Turnstile/会话、表单/表格/配置、金额精度、状态/错误、实时恢复、路由、构建测试门禁、运行时配置，以及 spot/margin/seconds/wallet/admin 业务意图；与 2026-08-24 前端审计对照但以当前代码为准。
- Scope: internal（当前仓库静态跨层审计；未访问生产环境或外部网络）
- Date: 2026-08-30

## Findings

### 口径与摘要

- 优先级沿用当前任务口径：P0 仅用于直接资金、权限、结算、价格时点或不可恢复数据正确性；P1 用于业务可用性、跨进程可靠性和显著维护风险；P2 用于体验、性能、一致性及长期治理（`.trellis/tasks/08-30-project-code-business-optimization-reaudit/prd.md:15-20`）。
- 运行时标记：**静态成立**表示控制流/DTO 已足以证明；**待运行时复现**表示仍应以浏览器、原生包或真实 fixture 量化影响；**待生产补证**表示暴露频率依赖生产数据或配置。
- 本轮保留 **2 项 P0、7 项 P1、1 项 P2**。最紧急的是：PC 仍用客户端拼装的杠杆风险代替服务端权威快照，以及秒合约结算价在 PC 被固定成 0。

### 历史前端审计复核

| 2026-08-24 旧项 | 当前结论 | 当前证据 |
|---|---|---|
| P0-01 平仓方向相反 | **已完成** | `pc/src/domain/marginActions.ts:53-68` 按 `close_long/close_short` 精确找仓位；`pc/src/components/trade/ContractOrderForm.vue:463-475` 提交真实 position ID；测试见 `pc/tests/contract-margin-actions.test.ts:23-31`。 |
| P0-02 UI 承诺部分/限价但后端全平 | **旧结论已失效，PC 当前安全但功能落后** | PC 只发空对象全平：`pc/src/api/contract.ts:30-33,80-92`；后端现已新增 `percentage + idempotency_key`：`src/modules/margin/presentation.rs:60-67`、`src/modules/margin/application/lifecycle.rs:333-368`。 |
| P0-03 固定 isolated | **已完成** | capability/setting 交集在 `pc/src/domain/marginActions.ts:31-38`，请求保留当前模式在 `pc/src/api/backendAdapters.ts:1569-1582`。 |
| P0-04 客户端编造杠杆风险 | **仍存在，见 APC-P0-01** | 当前 mapper 仍忽略 `cross_accounts` 并生成旧钱包模型。 |
| P0-05 提现网络硬编码且后端不校验 | **后端部分已完成；PC 新增契约断裂，见 APC-P1-01** | 后端 quote 会锁定并校验网络/费率；PC 未调用 quote，withdraw 网络仍走硬编码。 |
| P0-07 加载失败呈现空/旧数据 | **仍存在，见 APC-P1-02** | margin/seconds/spot 多处 catch 后只写 console。 |
| P0-08 批量平仓部分失败提示成功 | **已完成** | `pc/src/components/trade/ContractOrders.vue:417-444` 区分 success/partial/failure 并保留 failures。 |
| P1-01 会话双事实与 redirect 丢失 | **仍存在，见 APC-P1-04** | token/store 仍分裂，Admin/PC 登录仍忽略来源路由。 |
| P1-02 实时沉默检测 | **仍存在，见 APC-P1-05** | 仅在 close 时固定延迟重连，无 heartbeat/watchdog/freshness。 |
| P1-03 Admin ANY 动作权限 | **仍存在，见 APC-P1-03** | 资源级 `.some(...)` 仍控制整组按钮。 |
| P1-04/05 Admin transport、弱类型与 mega-chunk | **仍存在/部分改善** | 204 已处理且表格能区分错误与空；timeout、mutation retry、`ApiRecord`、共享 `resourceConfigs` chunk 仍在。 |
| P1-06/07 PC 生产回退与交付门禁 | **仍存在/部分完成，见 APC-P1-07、08** | 已新增 CI P0 gate，但只运行 1 个 PC 测试文件，且不构建 PC/Tauri。 |
| P1-09 核心交易契约未生成 | **仍存在，见 APC-P1-07** | OpenAPI 仍手工维护且没有前端生成脚本。 |
| P1-12 杠杆历史类型/时间错误 | **部分完成** | order type 已映射；`createTime` 仍固定 0：`pc/src/api/backendAdapters.ts:1585-1603`。 |

### APC-P0-01 — PC 杠杆页面仍绕过服务端权威风险快照

- **优先级 / 分类**：P0；资金风险与价格/风险数据正确性；历史 P0-04 未闭环。
- **证据**：后端 `/margin/wallets` 明确返回 `wallets`、`positions`、`cross_accounts`，其中包括 equity、unrealized PnL、maintenance margin、margin ratio（`src/modules/margin/presentation.rs:207-229`）；单仓权威实时风险端点为 `/margin/positions/:id/risk`（`src/modules/margin/routes.rs:333-353`），完整风险 DTO 在 `src/modules/margin/presentation.rs:442-538`。PC 虽声明 `BackendMarginCrossAccount`（`pc/src/api/backendAdapters.ts:774-788`），但 `mapMarginWalletsToContractWallets` 仅拼接 wallet 与 position（`:1616-1659`），没有消费 `cross_accounts`。position mapper 把 `margin_amount` 当余额、entry price 当 current price，并把 close fee/maintenance rate 置 0（`:1883-1918`）；store 又把 0 替换为 0.0001/0.005（`pc/src/stores/contract.ts:265-327`）；UI 最终按 WS 价格自行计算 PnL 与全仓 ratio（`pc/src/components/trade/ContractOrders.vue:237-321`）。PC 源码未发现调用 `/:id/risk`。
- **影响**：多交易对、对冲、利息或全仓场景可展示与强平/转账事实源不同的 equity、PnL、维持保证金率和风险比；服务端仍保护资金动作，但用户可能依据错误风险卡做平仓或补保证金决策。
- **建议修复**：新增强类型 PC risk read model；钱包总览直接消费 `cross_accounts`，仓位卡调用 `/:id/risk` 并保留 `observed_at`/mark 时间；删除客户端权威风险公式和 0.0001/0.005 默认值，缺失或陈旧统一显示“风险不可用/已陈旧”。
- **兼容策略**：先在 adapter 中并行新增 nullable `authoritativeRisk`，旧字段仅作非风险布局兼容；UI 切换后再移除旧公式，不修改现有资金写接口。
- **验证**：构造同保证金币种、多 pair、long/short 混合与利息 fixture，逐字段比对 PC 与后端 risk/cross snapshot；缺失、60 秒以上陈旧或任一 mark 失败时不得显示 0 或本地估算。
- **工作量 / 依赖**：L，5–8 天；依赖稳定风险 DTO、PC Decimal/显示组件和 WS freshness 状态。
- **运行时证据**：**静态成立**；具体误差值与用户暴露量 **待运行时 fixture/生产补证**。

### APC-P0-02 — PC 丢弃秒合约结算价与行情证据，历史 close price 恒为 0

- **优先级 / 分类**：P0；结算与价格时点数据正确性。
- **证据**：后端订单响应包含 `settlement_price`、tick ID、source、observed_at、generation、version（`src/modules/seconds_contract/presentation.rs:242-297`），契约要求结算价持久化并在订单响应中返回（`.trellis/spec/backend/seconds-contracts.md:65-87,130-163`）。PC `BackendSecondsOrder` 未声明这些字段（`pc/src/api/backendAdapters.ts:655-675`），mapper 固定 `closePrice: 0` 并按 stake × payout rate 在 JS 本地算 profit（`:1515-1535,1847-1850`）；历史表直接渲染这些值（`pc/src/views/SecondOptions.vue:512-539`）。
- **影响**：已结算订单的关键复核价格显示为 0，无法验证 win/loss 所用价格及其来源；本地 Number 计算也可能与服务端资产精度后的实际入账不一致。
- **建议修复**：PC DTO/mapper 原样保留 settlement 字段；历史表展示精确结算价、来源和观测时间。后端读模型宜加 additive `payout_amount/net_pnl`，PC 直接展示服务端 Decimal 文本，不自行重算入账结果。
- **兼容策略**：新增字段均为 additive；旧记录缺字段时显示 `--`，不得回退 0 或当前行情。
- **验证**：settled win/loss/tie（若支持）与未结算 fixtures；断言 entry/settlement/source/observed_at 逐字段一致，显示 PnL 等于钱包/ledger 实际量，旧 payload 显示未知。
- **工作量 / 依赖**：M，2–4 天；若增加 payout/net 字段需后端 DTO、SQL 和 contract fixture。
- **运行时证据**：**静态成立**；无需生产数据即可证明 closePrice 恒 0。

### APC-P1-01 — 当前 PC 提现请求缺必填 quote_id，提现提交与后端不兼容

- **优先级 / 分类**：P1（完整资金业务不可用，但服务端在动账前拒绝，不提升为 P0）；API 契约回归。
- **证据**：后端 `CreateWithdrawalRequest.quote_id` 为必填（`src/modules/wallet/presentation.rs:65-97`），路由先提供 quote 再创建申请（`src/modules/wallet/routes.rs:224-251`）；创建流程加载 quote 并校验 owner、network、amount、fee、fingerprint、expiry/config version（`src/modules/wallet/application.rs:734-805`）。OpenAPI 也已登记必填 `quote_id`（`src/openapi/wallet.rs:8-40`）。PC 请求类型和 mapper 均无 quote_id（`pc/src/api/backendAdapters.ts:124-145,1245-1256`），`submitWithdraw` 直接 POST（`pc/src/api/wallet.ts:286-294`），且 withdraw 网络不查询后端配置而回退硬编码列表（`:163-187,297-319`）。现有测试还断言旧无 quote payload（`pc/tests/backendAdapters.test.ts:431-469`）。
- **影响**：当前后端会在反序列化/校验阶段拒绝 PC 提现；即使 UI 展示本地手续费，也没有服务器 quote 的费用、净额、有效期和配置版本保证。拒绝发生在冻结前，因此未发现错误动账旁路。
- **建议修复**：PC 增加 quote API/state，选择网络和输入金额后获取权威 fee/net/total_reserved/expiry；确认时提交 quote_id 与同一参数。提供真实 withdrawal-network catalog，禁止生产硬编码回退；修正旧测试。
- **兼容策略**：在新客户端上线前禁用提交并显示“需要更新”，不要降级绕过 quote；服务端继续强制 quote。若新增 network catalog，先 additive 上线再切换客户端。
- **验证**：正常、过期、配置变化、异参重放、禁用网络、重复点击 fixtures；断言成功只冻结 quote.total_reserved，失败零申请/零账本/零余额变化；PC 请求必须含 quote_id。
- **工作量 / 依赖**：M，3–5 天；依赖 withdrawal network 读接口或明确复用契约、PC quote UI、端到端测试。
- **运行时证据**：**静态成立**；实际 HTTP 状态码 **待本地集成复现**。

### APC-P1-02 — PC 将加载失败、最新 N 条接口和页面模型混为“空/旧数据”

- **优先级 / 分类**：P1；业务可用性、状态/错误与审计历史完整性；历史 P0-07/P1-12 部分延续。
- **证据**：margin store 的 coins/current/history/wallet loads catch 后仅 console，不设置 error/stale（`pc/src/stores/contract.ts:117-146,172-219,221-263,265-327`）；spot 订单加载失败同样只 console（`pc/src/components/trade/OrderHistory.vue:197-218`）；seconds loads 也只 console（`pc/src/stores/second.ts:155-184,187-231`）。seconds API 忽略 pageNo/pageSize（`pc/src/api/second.ts:31-35,74-76,99-105`），store 却在下一页把同一 latest 列表追加（`pc/src/stores/second.ts:187-226`），而后端明确只支持 limit、不支持 offset（`src/modules/seconds_contract/presentation.rs:15-21`、`src/modules/seconds_contract/routes.rs:124-136`）。margin mapper 的 `createTime` 仍固定 0（`pc/src/api/backendAdapters.ts:1585-1603`），历史表会显示 `--`（`pc/src/components/trade/ContractOrders.vue:105-126,334-336`）。
- **影响**：断网/403/5xx 可被用户误认为没有订单，旧值无陈旧标识；秒合约滚动可能重复最新记录且无法访问更老订单；杠杆历史缺创建时间，客服/用户无法可靠核单。
- **建议修复**：所有 store 使用显式 loading/error/stale/lastSuccessfulAt；失败保留旧数据并标陈旧，不覆写为空。为 seconds/margin/spot 用户历史增加 cursor/offset + total 契约，或在后端扩展前移除伪 load-more；补齐 margin `created_at/opened_at`。
- **兼容策略**：旧 latest-only 响应保留；新分页 envelope/additive cursor 由新版 PC 使用。错误状态先在 store facade 增加，不改变组件调用签名。
- **验证**：网络失败、401 refresh 失败、50/51/120 条历史、翻页重复、乱序/重复 ID、margin market/limit 时间 fixtures；断言错误不显示“暂无数据”，跨页无重复且可访问最老记录。
- **工作量 / 依赖**：L，5–8 天；依赖后端分页 DTO/索引和 PC 通用 async state。
- **运行时证据**：控制流 **静态成立**；重复滚动与真实历史规模 **待浏览器/数据库 fixture**。

### APC-P1-03 — Admin 只按资源级 ANY 权限显示整组高风险按钮

- **优先级 / 分类**：P1；权限体验与运营流程正确性（服务端未形成越权）。
- **证据**：前端把 endpoint 扩成 write/review/settle/operate 四种候选（`web/src/admin/access.tsx:158-162`），`ResourcePage` 用 `.some(...)` 命中任一权限后同时暴露 actions/batchActions/rowActions（`web/src/admin/resources/resourceConfigs.tsx:1453-1467`）。提现同一行内混合 approve/reject/broadcast/confirm/fail（`web/src/admin/resources/actions/wallet.tsx:1269-1303`）。后端按 HTTP 路径精确映射具体 action，未映射 fail closed（`src/modules/admin/service/access_control.rs:88-104,242-264`）。
- **影响**：review-only、write-only、settle-only 角色会看到实际无权按钮并在提交后收到 403；虽然后端守住权限，但高风险运营页面的可执行意图和审计职责分工不真实。
- **建议修复**：每个 action descriptor 声明精确 permission；创建、行操作、批量操作和快捷入口分别过滤，后端继续作为最终权威。
- **兼容策略**：纯前端 additive metadata；未标注 action 默认隐藏而非继承资源 ANY，超级管理员行为不变。
- **验证**：read/write/review/operate/settle/`*` 六类角色 fixture 逐按钮断言；隐藏动作不能发请求，直接伪造请求仍由后端 403。
- **工作量 / 依赖**：M，2–4 天；依赖共享 permission catalog/action descriptor。
- **运行时证据**：**静态成立**；角色分布 **待生产补证**。

### APC-P1-04 — PC 会话仍有双事实源，PC/Admin 登录均丢失原目标；PC Turnstile reset 会遗失 widget 句柄

- **优先级 / 分类**：P1；认证可用性与会话一致性。
- **证据**：PC Pinia 以 `token` ref 判定登录（`pc/src/stores/user.ts:19-45`），`useAuthRequired` 却用没有响应式依赖的 `computed(() => Boolean(readAuthToken()))`（`pc/src/composables/useAuthRequired.ts:8-28`）；401 refresh 只写 localStorage（`pc/src/api/request.ts:104-139`）。PC `goToLogin` 写入 redirect，但成功后固定 `router.push('/')`（`pc/src/views/auth/Login.vue:421-430`）。PC Turnstile `reset()` 后把 widget ID 置 null（`:205-230`），后续 initialize/remove 无法回收已 reset 的旧 widget（`:260-305,326-327,370-372`）。Admin guard 仅保存 pathname（`web/src/auth/RequireAdmin.tsx:7-17`），登录成功固定跳 dashboard（`web/src/auth/LoginPage.tsx:129-142`）。
- **影响**：refresh/logout/跨组件状态可能短暂不一致；深链接登录后丢失上下文；Turnstile 重试、缺 token 或卸载路径可能留下 orphan widget/iframe，造成重复挑战或无法恢复。
- **建议修复**：以单一响应式 session store 持有 access/refresh token，storage 仅做持久化并支持 storage event；refresh 原子更新 store 与私有 WS。统一校验站内 redirect 并由登录页消费；PC 复用 Admin 已有 generation-aware Turnstile lifecycle。
- **兼容策略**：一个版本双读旧 `token/refresh_token/user`，写新 session 后迁移并清理旧键；redirect 只接受同源内部路径。
- **验证**：刷新单飞、刷新后 store/HTTP/private WS token 一致、登出、跨标签页、401 失败、带 query/hash 深链接；Turnstile slow load/reset throw/unmount/重试后仅一个 script、widget、iframe。
- **工作量 / 依赖**：M–L，4–7 天；依赖共享 auth session facade 与 PC Turnstile 单测/浏览器测。
- **运行时证据**：双事实与 redirect **静态成立**；orphan iframe 数量 **待浏览器复现**。

### APC-P1-05 — Admin/PC 实时链路只处理显式 close，缺半开检测与可信 freshness

- **优先级 / 分类**：P1；跨进程可靠性与行情可用性。
- **证据**：PC socket 只在 `onclose` 后固定延迟重连（`pc/src/api/stomp.ts:103-147,426-442`），收到 `pong` 只忽略（`:327-373`），没有发送 ping、入站 watchdog、指数退避/jitter、online/visibility recovery 或 freshness 状态；后端已支持 text ping/pong（`src/modules/events/service/websocket.rs:332-350,409-422`）。Admin 每次 `subscribeMarketTicker` 都创建独立 socket，只有 message/cleanup，没有 open/error/close/reconnect（`web/src/api/marketTickerSocket.ts:51-69`）；`MarketPairLatestPrice` 每行独立订阅且丢弃 observedAt（`web/src/admin/resources/resourceConfigs.tsx:506-519`），服务端页默认 50 行（`web/src/admin/resources/AdminResourcePage.tsx:65-68,181-184`）。
- **影响**：TCP 半开或代理静默时持续展示陈旧行情；PC 私有流在 token 轮换后不会主动重连；Admin 市场页可能建立多达当前行数的 socket，且价格冻结无提示。
- **建议修复**：建立单一 connection manager：heartbeat、独立入站沉默 watchdog、指数退避+jitter、generation、online/visibility 恢复、token-change reconnect、topic 引用计数；Admin 用共享 multiplex socket，并向 UI 暴露 live/stale/offline 和 observedAt。资金状态继续用 REST 对账，WS 只作刷新提示。
- **兼容策略**：保留现有 `subscribe(...)->unsubscribe` facade，内部替换 manager；逐页面迁移并设置并发连接预算。
- **验证**：OPEN 后静默、断网/恢复、token 轮换、旧 socket 迟到消息、最后订阅释放、50 行市场表；断言单连接/正确重订阅/无遗留 timer，陈旧价格不得显示 live。
- **工作量 / 依赖**：L，5–8 天；依赖共享 WS lifecycle、可观测 connection state 与 fake-timer tests。
- **运行时证据**：缺失机制 **静态成立**；代理半开与连接数 **待浏览器/网络故障注入**。

### APC-P1-06 — 手写 DTO 与不完整 PC 门禁未阻止已发生的契约漂移

- **优先级 / 分类**：P1；API 边界、生成/共享类型与测试治理。
- **证据**：`src/openapi.rs:1-4` 明示文档与真实路由需手工同步；路径注册覆盖 auth/wallet 等，但未覆盖 spot/margin/seconds 主交易契约（`:53-187`）。wallet OpenAPI 已含 quote_id（`src/openapi/wallet.rs:8-40`），PC 手写 DTO 仍漏字段，证明文档没有进入编译链。Admin 通用类型仍是 optional 字段袋与 `ApiRecord`（`web/src/api/types.ts:25-47`），错误 response key 会静默变成空数组（`web/src/api/adminResources.ts:35-48`）。PC 有 21 个 `pc/tests/*.test.ts` 文件，但 `pc/package.json:6-12` 只定义 `test:margin`；P0 gate 也只运行 type-check 与该单文件（`scripts/p0-release-gate.sh:22-24`），所以包含旧提现断言的 `backendAdapters.test.ts` 不会阻断 CI。
- **影响**：后端新增必填字段、枚举、Decimal 或分页形状时，Admin/PC 可编译通过并在运行时失败；当前提现断裂和 seconds settlement 字段丢失正是已发生实例。
- **建议修复**：补齐 spot/margin/seconds/wallet 用户与 Admin mutation OpenAPI；生成 transport-only TypeScript 包，领域 adapter 只做显示映射；CI 加 schema diff、生成物 freshness、所有 PC tests、PC Vite build 与关键 contract golden fixtures。Admin list 对缺 key/错误类型抛 typed contract error。
- **兼容策略**：生成类型先放新 namespace，与手写 adapter 并行；按 wallet→seconds→margin→spot 迁移，保持 endpoint 和展示 domain model 不变。
- **验证**：故意删除 quote_id、改 settlement_price 类型、改 failures 枚举或 response key 时，生成检查/类型检查/contract test 必须在 PR 阶段失败；所有 21 个 PC 测试由标准 `test` script 运行。
- **工作量 / 依赖**：L–XL，8–15 天，可按领域拆分；依赖 OpenAPI 覆盖、生成工具和 CI 缓存。
- **运行时证据**：**静态成立**；无需生产补证。

### APC-P1-07 — 金额边界仍把 18 位 Decimal 文本转换为 JS Number，并按 2–6 位展示

- **优先级 / 分类**：P1；金额/精度与操作展示（若生产资产实际允许高精度资金输入，应重新评估为 P0）。
- **证据**：后端资产精度允许 0..=18，存储为 DECIMAL(38,18)（`.trellis/spec/backend/wallet-amount-precision.md:10-22`）。Admin `formatAdminNumber` 先经 numeral/Number 语义并最多展示 6 位（`web/src/shared/numberFormat.ts:1-3,50-60`），测试只覆盖 6 位（`web/src/shared/format.test.tsx:27-53`）。PC 通用 formatter 先 `Number(value)`，金额通常只显示 2/4 位、低价最多 6 位（`pc/src/utils/format.ts:5-18`）；spot/margin/seconds/wallet 写请求广泛以 `amount: number` 再 `String(...)` 发送，例如 `pc/src/api/backendAdapters.ts:1207-1228,1499-1512`、`pc/src/api/contract.ts:137-144`。
- **影响**：小额余额/费率可显示为 0 或被过度舍入；超过 IEEE-754 安全有效位的输入可能在发送前改变用户意图。当前服务端精度校验能拒绝部分非法值，但不能识别一个已被客户端改成另一合法十进制的值。
- **建议修复**：transport/domain 金额统一保留 decimal string；输入使用字符串 + 资产 precision 校验，计算使用 Decimal 库；展示按资产 precision/业务列配置并显式标注截断/舍入规则。
- **兼容策略**：adapter 先接受 `string | number`，内部立即规范化为 string；组件逐步改为 DecimalText，禁止新资金 API 使用 number。
- **验证**：0/2/8/18 scale、极小非零、超过 2^53、阶梯边界、PnL/fee fixtures；序列化前后字符串相等，Admin/PC 不显示虚假 0，服务端 ledger 与确认值一致。
- **工作量 / 依赖**：L，5–10 天；依赖 Decimal 库、资产 precision 元数据和统一 formatter。
- **运行时证据**：转换/格式化 **静态成立**；实际 P0 可达性 **待生产资产 precision/config 补证**。

### APC-P1-08 — `unknown_broadcast` 在后端是关键冻结态，但 Admin/PC 与 Dashboard 未完整呈现

- **优先级 / 分类**：P1；提现运营可见性与状态契约。
- **证据**：后端允许并持久化 `unknown_broadcast`（`src/modules/wallet/application.rs:1213-1236`），该状态保持 frozen 并进入查询/人工复核路径（`src/modules/wallet/infrastructure/withdrawals.rs:889-944`）。Admin 状态 map/filter 缺该值（`web/src/admin/resources/resourceConfigs.tsx:96-105,254-259`），Dashboard pending 计数也漏掉它（`src/modules/admin/infrastructure/dashboard_audit.rs:85-103`）；PC 只原样回显未知字符串且没有专用语义（`pc/src/views/User/Withdraw.vue:393-415`）。
- **影响**：运营无法从筛选器或 Dashboard 数量发现处于广播歧义、资金仍冻结的请求；用户看到内部英文状态，不能区分自动重试与人工复核。资金本身仍由后端安全冻结。
- **建议修复**：把 withdrawal status 定义为共享生成 enum；Admin 加中文标签、筛选、待处理计数与只读详情；PC 加用户可理解状态说明。Dashboard 查询将 unknown_broadcast 纳入 pending/attention 指标，并单列数量更佳。
- **兼容策略**：纯 additive 状态展示/统计；不得把 unknown 映射成 failed 或自动解冻。
- **验证**：九状态 fixtures 覆盖列表、筛选、Dashboard、PC label/class；unknown 状态不得出现 fail/confirm 等不合法动作，金额持续 frozen。
- **工作量 / 依赖**：S–M，1–2 天；依赖共享状态 enum/文案与 Dashboard 测试。
- **运行时证据**：**静态成立**；生产中当前数量 **待生产补证**。

### APC-P1-09 — PC/Tauri 交付与运行时配置仍可产出错误指向、不可更新且未验证的制品

- **优先级 / 分类**：P1；构建、原生运行时与发布可靠性。
- **证据**：PC 缺环境变量时静默回退真实生产域名（`pc/src/config/app.ts:1-6`）。Tauri beforeBuild 只跑 `npm run build`，而 PC build 只是 Vite、不含 type-check（`pc/src-tauri/tauri.conf.json:6-10`、`pc/package.json:6-12`）；CSP 为 null，updater pubkey 仍是 placeholder（`pc/src-tauri/tauri.conf.json:24-45`）。前端 updater 调用 updater/dialog/process（`pc/src/components/common/AppUpdater.vue:2-49`），Rust 仅注册 updater/dialog（`pc/src-tauri/src/lib.rs:3-16`），Cargo 没有 process plugin（`pc/src-tauri/Cargo.toml:20-27`），capability 又只有 `core:default`（`pc/src-tauri/capabilities/default.json:1-10`）。P0 gate 不执行 PC build/Tauri smoke（`scripts/p0-release-gate.sh:22-24`），Docker 也只构建 Admin web（`Dockerfile:6-17,61-64`）。
- **影响**：错误环境的 PC 制品可能直连真实租户；更新检查/对话框/重启可能因签名、插件注册或 capability 失败且仅写 console；类型检查通过不代表 Web/Tauri 构建可交付，CSP 也缺少原生壳防护基线。
- **建议修复**：production/native 构建强制合法 backend origin 并输出可追溯 manifest；注册 process Rust plugin、最小 updater/dialog/process capabilities、真实签名公钥与 endpoint，设置最小 CSP；CI 按目标 OS 运行 type-check、全测、Vite build、`tauri build`/smoke。
- **兼容策略**：先建立 staging updater channel 和签名轮换/回滚；已有 0.1.0 制品保留旧 endpoint 一段窗口，但禁止 placeholder key 发布。
- **验证**：缺失/非法/http/loopback origin 构建失败；三平台签名更新、取消、下载、安装、relaunch、回滚 smoke；capability deny/allow 测试；制品 SBOM/manifest 记录 API/WS origin。
- **工作量 / 依赖**：M–L，4–7 天；依赖代码签名密钥、更新托管、三平台 runner。
- **运行时证据**：配置断裂 **静态成立**；原生 updater 实际错误与 CSP 暴露 **待打包运行时验证**。

### APC-P2-01 — 当前 PC 能力曝光仍与后端产品意图不一致

- **优先级 / 分类**：P2；业务一致性与转化体验。
- **证据**：seconds 产品目录是公开接口，spec 明确 guest PC 应展示真实产品/行情（`src/modules/seconds_contract/routes.rs:87-97`、`.trellis/spec/backend/seconds-contracts.md:159-163`），但 PC mounted 时未登录立即 return，模板整页显示 AuthRequired（`pc/src/views/SecondOptions.vue:310-339,348-352`）；现有测试名称宣称“keeps public market data”却实际断言整页 guard（`pc/tests/guest-auth-states.test.ts:69-80`）。另一方面，后端已支持 1..=100 百分比幂等平仓（`src/modules/margin/presentation.rs:60-67`），PC 仍只发 `{}` 全平（`pc/src/api/contract.ts:80-92`）。
- **影响**：访客看不到本应公开的秒合约产品，降低发现/转化；已存在的安全部分平仓能力没有进入 PC。当前全平语义明确且安全，不属于执行意图错误。
- **建议修复**：seconds 页面始终加载公开产品/行情，仅保护余额、下单和私有历史；修正误导测试。部分平仓以服务端 capability 控制，UI 清楚显示比例、幂等重试与剩余仓位，不恢复旧限价伪能力。
- **兼容策略**：guest 渲染为 additive；全平保持 100% 默认，部分平仓仅在 capability 明确开启时出现。
- **验证**：guest 可看公开产品/行情但私有请求为 0；登录 redirect 返回原 symbol。1/37/100% 请求、重放、剩余仓位/风险刷新与全平旧客户端兼容测试。
- **工作量 / 依赖**：M，3–5 天；依赖公开/私有页面状态拆分与 margin close capability 字段。
- **运行时证据**：**静态成立**；转化影响 **待产品埋点补证**。

### 业务保护矩阵

| 领域 | 当前强项 | 当前主要缺口 |
|---|---|---|
| Spot | PC 创建订单携带幂等键（`pc/src/api/exchange.ts:27-41`）；批量撤单保留成功/失败数量（`:58-69`）；服务端市价执行价取新鲜 Redis，客户端价只作滑点参考（`src/modules/spot/application/order_creation.rs:36-123,208-226`）。 | PC 金额仍是 Number；历史为 latest-N 聚合且缺真实分页；实时 freshness 不可信。 |
| Margin | 平多/平空精确 position ID、mode capability 交集、批量部分失败均已修复；转出 authority 缺失时 fail closed（`pc/tests/contract-margin-actions.test.ts:23-95,165-235`）。 | APC-P0-01 风险展示仍不权威；历史时间缺失；后端部分平仓尚未曝光。 |
| Seconds | 下单只发 product/cycle/direction/stake，服务端取权威开仓价；余额复用 spot wallet 且 PC 无虚构 transfer（`pc/tests/second-options-transfer.test.ts:13-50`）。到期后还有 REST result polling（`pc/src/views/SecondOptions.vue:218-273`）。 | APC-P0-02 结算价丢失；APC-P1-02 假分页；guest 公开目录被阻断。 |
| Wallet | 后端 quote、精度/费率版本、network 配置、歧义广播冻结与幂等状态机是显著增强。 | APC-P1-01 PC 未接 quote；APC-P1-08 unknown 状态不可见；高精度仍经 Number。 |
| Admin | 受保护路由与 read permission boundary 完整（`web/src/app/router.tsx:9-55`、`web/src/admin/routes.tsx:9-54`）；设置编辑器保留 baseline/draft/409 草稿（`web/src/admin/settings/useAdminSettingsEditor.ts:50-113`），保存前展示 diff/影响/必填原因（`web/src/admin/settings/SettingsSaveConfirmation.tsx:21-149`），离页 guard 完整（`web/src/admin/settings/UnsavedChangesGuard.tsx:9-72`）；DataTable 已区分 loading/error/empty（`web/src/shared/DataTable.tsx:101-125`）。 | 动作权限粒度、transport timeout/retry、弱 DTO、金额精度、unknown 状态与 realtime fan-out 仍需收口。 |

## Strengths

1. **Admin Turnstile 已达到明确生命周期基线**：单例 loader、失败重试、generation、旧 callback 隔离、reset/remove 均集中在 `web/src/auth/turnstile.ts:19-124,156-260`，组件测试覆盖 scope 切换、2FA 清理和 unmount（`web/src/auth/LoginPage.test.tsx:85-193`）。这可直接作为 PC 修复模板。
2. **后端权限继续 fail closed**：`src/modules/admin/service/access_control.rs:88-104` 对未登记业务路由返回 `admin.unmapped`，前端权限问题不会升级为服务端越权。
3. **Admin 配置变更工作流成熟**：baseline/draft/conflict、写入 retry=false、差异确认、保存原因和离页保护形成完整闭环（`web/src/admin/settings/query.ts:12-25` 与上表文件）。
4. **PC HTTP transport 优于旧审计基线**：Axios 有 10 秒 timeout、401 refresh 单飞和一次 replay，auth routes 不递归刷新（`pc/src/api/request.ts:43-56,104-139,147-199`）。
5. **服务端已收口关键资金事实源**：spot 执行价、margin 部分平仓/转出风险、seconds 开仓与结算、wallet quote/unknown broadcast 均由服务端校验；本轮主要风险集中在 PC 读模型和交付门禁没有跟上，而非后端重新信任客户端。

## Files Found

- `.trellis/tasks/archive/2026-08/08-24-project-architecture-business-flow-audit/research/frontends-cross-layer.md` — 2026-08-24 前端审计基线。
- `.trellis/tasks/08-30-project-code-business-optimization-reaudit/prd.md` — 当前复审范围、优先级和证据要求。
- `web/src/api/client.ts`、`web/src/app/providers.tsx` — Admin HTTP、refresh、错误解析与 React Query 默认重试。
- `web/src/api/types.ts`、`web/src/api/adminResources.ts` — Admin 弱类型资源边界。
- `web/src/admin/access.tsx`、`src/modules/admin/service/access_control.rs` — 前后端权限推导与最终授权。
- `web/src/admin/resources/resourceConfigs.tsx`、`web/src/admin/resources/actions/wallet.tsx` — 资源表、状态、行情单元格和高风险动作。
- `web/src/auth/turnstile.ts`、`web/src/auth/LoginPage.tsx`、`web/src/auth/RequireAdmin.tsx` — Admin Turnstile 与会话导航。
- `web/src/admin/settings/`、`web/src/shared/DataTable.tsx` — 配置编辑状态机与表格状态组件。
- `pc/src/api/request.ts`、`pc/src/utils/authStorage.ts`、`pc/src/stores/user.ts` — PC HTTP refresh 与会话持久化。
- `pc/src/api/backendAdapters.ts` — PC 手写后端 DTO 与旧展示模型适配中心。
- `pc/src/api/contract.ts`、`pc/src/stores/contract.ts`、`pc/src/components/trade/ContractOrderForm.vue`、`ContractOrders.vue` — 杠杆动作、风险展示与历史。
- `pc/src/api/second.ts`、`pc/src/stores/second.ts`、`pc/src/views/SecondOptions.vue` — 秒合约目录、订单、结算展示与分页。
- `pc/src/api/wallet.ts`、`pc/src/views/User/Withdraw.vue` — PC 提现网络、费用预览、提交和状态。
- `pc/src/api/stomp.ts`、`web/src/api/marketTickerSocket.ts` — PC/Admin 实时连接实现。
- `pc/src-tauri/tauri.conf.json`、`pc/src-tauri/capabilities/default.json`、`pc/src-tauri/src/lib.rs`、`pc/src/components/common/AppUpdater.vue` — Tauri 构建、安全能力和更新链路。
- `src/modules/margin/presentation.rs`、`src/modules/margin/routes.rs`、`src/modules/margin/application/lifecycle.rs` — 杠杆权威 DTO、风险与部分平仓。
- `src/modules/seconds_contract/presentation.rs`、`src/modules/seconds_contract/routes.rs` — 秒合约结算与 latest-only 用户列表契约。
- `src/modules/wallet/presentation.rs`、`src/modules/wallet/application.rs`、`src/modules/wallet/infrastructure/withdrawals.rs` — 提现 quote、冻结和歧义广播事实源。
- `src/openapi.rs`、`src/openapi/wallet.rs` — 当前手工 OpenAPI 聚合与提现 schema。
- `scripts/p0-release-gate.sh`、`.github/workflows/docker-image.yml`、`Dockerfile` — 当前测试与交付边界。

## Code Patterns

- **权威 DTO 被旧 UI 模型吞掉**：后端 Decimal/risk/settlement 字段 → `backendAdapters.ts` 转 Number 或丢字段 → store 填默认值 → UI 再计算。
- **latest-only 被包装成 page**：API 只返回最新 limit → PC 接口忽略 pageNo → store append → 重复/不可达历史。
- **错误即空或旧值**：catch 只 console，组件没有 error/stale/freshness 状态；Admin DataTable 已有正确反例。
- **资源级能力代替动作级权限**：endpoint → 多个候选 permission → `.some` → 整组按钮；后端仍逐动作精确检查。
- **连接关闭才恢复**：socket OPEN 但沉默没有 watchdog，observed_at 未进入 UI freshness。
- **文档存在但不参与编译**：wallet OpenAPI 已有 quote_id，PC 手写 DTO 与测试仍保持旧结构。

## External References / Versions

- 未联网检索外部资料；结论只依赖当前仓库代码、生成 schema、manifest 和 Trellis specs。
- `web/package.json:14-40`：React 19.2.6、React Router 7.16、TanStack Query 5.100.14、Semi 2.99.2、Vite 8、Vitest 4。
- `pc/package.json:14-56`：Vue 3、Pinia 2、Axios 1.6、Tauri JS 2、Vite 5；原生 Rust 侧见 `pc/src-tauri/Cargo.toml:17-27`。
- `Cargo.toml:48-49`：utoipa 5、utoipa-swagger-ui 8；仓库具备 OpenAPI 基础但核心交易覆盖和客户端生成门禁不完整。

## Related Specs

- `.trellis/spec/guides/cross-layer-thinking-guide.md` — 请求到持久化、异步、读模型和 UI 的跨层追踪。
- `.trellis/spec/guides/code-reuse-thinking-guide.md` — transport/domain/UI 复用边界。
- `.trellis/spec/admin/index.md`、`.trellis/spec/admin/auth-turnstile.md` — Admin 构建门禁与 Turnstile 生命周期。
- `.trellis/spec/backend/auth-sessions.md` — Bearer/refresh/private WS 会话合同。
- `.trellis/spec/backend/realtime-websockets.md` — 实时恢复、heartbeat 与消费语义。
- `.trellis/spec/backend/spot-orders.md` — spot 服务端价格与批量结果。
- `.trellis/spec/backend/margin-trading-actions.md` — margin capability、风险、部分平仓与批量结果。
- `.trellis/spec/backend/seconds-contracts.md` — seconds 时间、产品、公开目录、价格与精度。
- `.trellis/spec/backend/wallet-amount-precision.md` — 0..18 资产精度、费用与 ledger 一致性。

## Caveats / Not Found

- 本轮按用户要求只做静态读取并仅写本文件；未运行会写入 dist/cache/target 的 build/test，也未连接浏览器、Tauri 原生包、MySQL、Redis、链网关或生产配置。
- 行号基于 2026-08-30 当前工作树；已同时记录函数/组件符号，后续编辑可能导致行号偏移。
- 未实测 bundle 大小；只能静态确认所有通用资源路由共享 `resourceConfigs` import（`web/src/admin/routes.tsx:9-29`）且该文件 1469 行，因此不把具体性能损耗列为独立高优先级结论。
- Admin `.env` 使用 `VITE_BACKEND_API_DOMAIN`，client 实际读取 `VITE_API_BASE_URL`（`web/.env:1-2`、`web/src/api/client.ts:16`）；生产 Docker 采用同源 Nginx 时可能不受影响，开发/独立部署影响需运行时补证。
- 未核对生产中启用资产的实际 precision、withdrawal network、`unknown_broadcast` 数量、角色权限分布或 WS 代理超时，因此相关暴露频率均未写成事实。
