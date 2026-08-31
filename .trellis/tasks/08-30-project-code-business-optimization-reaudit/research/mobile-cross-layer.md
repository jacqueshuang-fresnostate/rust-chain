# Research: Mobile Vue/PWA/Tauri 跨层与核心业务复审

- Query: 对 `mobile/` 做有界静态复审，覆盖 API 契约、鉴权会话、金额精度、路由导航、公开/私有 WebSocket 与 REST 对账、状态归属、巨型视图/样式、可访问性/性能、测试与构建门禁，并复核钱包、现货、杠杆、秒合约、闪兑、借贷、理财、预测、新币和客服业务，以及历史 P1-06、P1-10、P1-15、P1-16 与客户端 P2。
- Scope: mixed（Mobile 内部代码 + 必要的后端路由/DTO/规范静态交叉核对）
- Date: 2026-08-30

## Findings

### 结论摘要与历史项状态

本轮只保留 8 条最高置信度发现。Mobile 已有若干可靠基线：安全内部 redirect、路由级懒加载、Axios 401 单飞刷新、提现 quote、Margin 能力 envelope/部分失败映射、公开行情租约与沉默恢复、Margin 私有提示后权威 REST 对账、Seconds 到期 REST 对账、Prediction/Convert quote-confirm、Loan/Earn/NewCoin 幂等创建、Support 幂等消息与五秒 REST 轮询。没有发现一个可由当前静态证据直接升级为新增 P0 的 Mobile 缺陷。

| 历史项 | 当前状态 | 本轮结论 |
| --- | --- | --- |
| P1-06 现货产品/批量契约 | **仍存在（Mobile 批量撤单子项）** | `cancelAllSpotOrders` 仍逐笔模拟并丢弃后端 `orders/failures`，见 MCL-P1-01；撮合产品语义由后端研究复核，本文件不重复计数。 |
| P1-10 实时事件/客户端收敛 | **部分完成** | Margin 和 Support 均以周期 REST 保证最终收敛，公开行情有专用活性机制；私有流仍只被 TradeView 局部持有、未消费 `support.refresh` 且无入站沉默 watchdog，见 MCL-P2-02。跨实例进程内广播根因仍属于后端 P1。 |
| P1-15 共享契约与行为测试 | **仍存在** | 资金 DTO 仍手写且宽松，金额先进入 JS `number`；80 个测试文件中 68 个读取源码文本，缺 Vue DOM 行为 harness，见 MCL-P1-02、03、05。 |
| P1-16 分层/巨型文件 | **仍存在** | `TradeView.vue` 已增至 6,089 行，仍同时拥有 REST、两类 WS、Margin 对账、业务动作、弹层和大段样式，见 MCL-P1-04。 |
| P2-03 前端包/样式边界 | **仍存在且规模未收敛** | 三个主要全局/共享样式文件合计 12,509 行；尚无 CSS/chunk budget，合并进 MCL-P1-04。 |
| P2 客户端交付项 | **部分改善** | CI 已执行 Mobile type-check 与全量 Node tests，但仍不构建 PWA/Tauri、不检查产物；Tauri CSP 仍为 `null`，见 MCL-P1-06。 |

### 核心业务覆盖快照

| 业务 | 当前保护 | 本轮剩余缺口 |
| --- | --- | --- |
| Wallet | 提现已使用 string quote、`quote_id`、费用版本和到期时间；Assets 请求有 session/version guard。 | 转账、快捷充值及大量读模型仍经 `number`；弱 DTO、行为测试与交付门禁共享 MCL-P1-02/03/05/06。 |
| Spot | 服务端价格/幂等键和单笔撤单可用。 | 批量撤单仍 N 次调用且无法表达部分失败；下单 quantity/price 先转 `number`。 |
| Margin | 能力交集、cross account、风险快照、部分平仓幂等、批量 failures、五秒权威账户对账均已落地。 | Mutation 金额仍是 `number`；全部生命周期仍集中在 TradeView；私有 socket 无沉默检测。 |
| Seconds | 公开产品、创建响应即时 upsert、到期 REST result 对账、独立行情 session 均存在。 | stake/min/max/payout 均转 `number`，页面达 2,818 行，DOM 交互测试缺失。 |
| Convert | MySQL 权威 quote-confirm 与客户端到期检查形成闭环。 | 输入/quote 金额转 `number`；页面请求没有统一 session-generation owner。 |
| Loan / Earn | 创建带幂等键，列表和产品来自 REST。 | 金额/费率宽松转 `number`，端点 DTO 缺严格运行时校验；页面生命周期模式不统一。 |
| Prediction | quote-confirm、服务端 order 响应即时落地，`outcome` 有局部严格校验。 | 其余必填 ID/金额/状态仍经宽松记录袋映射，stake 先转 `number`。 |
| New Coin | 订阅/购买带幂等键，生命周期与记录均来自 REST。 | 客户端用浮点 `quoteAmount / issuePrice` 生成 quantity，购买 price/quantity 与解锁费 amount 均先为 `number`。 |
| Support | MySQL/REST 权威、消息 client ID 重试复用、游标分页、五秒单飞轮询和 session generation 已实现。 | 后端已发 `support.refresh`，Mobile 私有流却只在 Contract TradeView 内监听 Margin 事件。 |

路由表为 Hash history、页面懒加载，登录消费 `sanitizeInternalRedirect`，Seconds/History、Product Hub、钱包和客服命名路由/回退关系静态一致；本轮未找到足够证据单列新的路由错误。页面普遍存在 44px、dialog focus trap、ARIA 和 reduced-motion 源码合同，但缺真实 DOM/浏览器门禁，归入 MCL-P1-05/06，而不把静态可访问性推测写成独立缺陷。

### MCL-P1-01 — Mobile 现货“全部撤单”仍绕过既有批量端点并抹平部分失败

- **严重级别 / 分类**：P1；API 契约、现货操作反馈；复核历史 P1-06。
- **静态证据**：`mobile/src/views/OrdersView.vue:266::cancelAll` 把当前订单 ID 交给 `cancelAllSpotOrders`；`mobile/src/api/trading.ts:210-215::cancelAllSpotOrders` 明写“后端暂未提供”，实际 `Promise.allSettled` 逐笔 DELETE 并只抛第一个 rejected。当前后端早已在 `src/modules/spot/routes.rs:47-60,170-184::cancel_all_orders` 注册 `DELETE /spot/orders`；`src/modules/spot/application/cancellation.rs:55-77::cancel_all_user_spot_orders_with_events` 返回完整 `orders` 与 `failures`，规范见 `.trellis/spec/backend/spot-orders.md:74-139`。
- **可达影响**：用户从 Orders 页执行一次“全部撤单”会产生 N 个 HTTP 请求；混合成功/失败时 Mobile 得不到全部失败 ID/code/message，无法给出准确剩余风险与重试范围。后端单笔事务仍保护余额，因此这是可用性/披露 P1，不是已证明的重复解冻 P0。
- **增量修复**：直接调用 `DELETE /spot/orders`，按可选 pair query 缩小范围，新增强类型 `SpotBatchActionResult { orders, failures }`，UI 按全成功/部分成功/全失败提示并随后读取权威订单列表。
- **兼容策略**：先保留 `cancelAllSpotOrders(...)` façade；内部切到批量端点，调用方逐步从 `Promise<void>` 迁移为结果对象。旧后端兼容若确有需要，应由显式 capability/version 决定，不能继续靠错误注释猜测。
- **验证**：mock 一次 DELETE 返回 2 success + 1 failure，断言网络请求数为 1、失败 ID 可见、成功订单移除、失败订单保留；重放不重复解冻；后端无批量能力时必须显式 fail closed。
- **工作量 / 依赖**：S，0.5–1 天；依赖 Mobile DTO/文案测试，无后端新能力依赖。
- **运行时 caveat**：未实际发请求或连接数据库；部分失败的生产频率未知，但路径与契约漂移静态成立。

### MCL-P1-02 — 资金输入仍先进入 IEEE-754 `number`，再被字符串化提交

- **严重级别 / 分类**：P1；金额/Decimal 契约。若生产资产实际开放 15–18 位有效数字或超安全整数金额，应重新评估更高优先级。
- **静态证据**：资产精度允许 `0..=18` 且后端使用 `DECIMAL(38,18)`（`.trellis/spec/backend/wallet-amount-precision.md:10-22`）；Mobile 规范也要求 mutation decimal 在 API 边界保持字符串（`.trellis/spec/mobile/backend-integration.md:316-318`）。但现货 `SpotOrderInput.price/quantity` 是 number 并在 `mobile/src/api/trading.ts:18-24,158-170` 再 `String(...)`；Margin `marginAmount` 同样见 `:26-35,250-265`，页面在 `mobile/src/views/TradeView.vue:1311-1424` 用 `Number(quantity.value)`；Seconds 见 `mobile/src/api/seconds.ts:55-70` 与 `mobile/src/views/SecondsView.vue:204-217,693-705`；Convert 见 `mobile/src/api/swap.ts:40-53`；Loan 见 `mobile/src/api/loan.ts:75-82`；Earn 见 `mobile/src/api/earn.ts:59-64`；Prediction 见 `mobile/src/api/prediction.ts:79-95`；New Coin 甚至在 `mobile/src/api/newCoin.ts:88-94` 用浮点除法生成 quantity，并在 `:163-176` 提交 number 派生的价格、数量和费用。Wallet 的快捷充值/转账也暴露 number 入参（`mobile/src/api/wallet.ts:500::createQuickRechargeOrder`、`:528::transferWalletFunds`）。提现 quote 使用 string 是正确反例。
- **可达影响**：极小金额、长尾小数或大有效位输入可在发请求前被改成另一个合法十进制；服务端精度校验只能拒绝超 scale，无法识别“客户端已改写但仍合法”的用户意图。展示余额转 number 还可能把非零值显示成 0，但本项核心是 mutation round-trip。
- **增量修复**：新增 `DecimalText` transport 类型和统一 normalize/precision validator；输入 ref 保持原始字符串，金额计算使用 Decimal 库，只有只读图表/百分比展示可转 number。按 wallet/spot→margin/seconds→convert/loan/earn/prediction/new-coin 分批迁移。
- **兼容策略**：适配层短期接受 `string | number`，进入边界立即规范化为 string，并对 number 调用记录 telemetry；端点与 JSON 字段名不变。不得一次性重写全部页面。
- **验证**：资产 scale 0/2/8/18、`0.000000000000000001`、超过 `2^53`、阶梯边界和最大余额 fixtures；断言输入字符串、确认页、请求 JSON 和服务端 ledger 精确一致。
- **工作量 / 依赖**：L，5–10 天；依赖 Decimal 库、资产 precision 元数据、MCL-P1-03 transport 类型收口。
- **运行时 caveat**：转换链静态成立；当前生产资产精度、真实余额范围和用户输入分布未读取，因此没有把每个路径都断言成已发生资金损失。

### MCL-P1-03 — 核心业务 DTO 仍是手写 `Record<string, unknown>` + 宽松默认值，契约漂移会静默变成 0/空值

- **严重级别 / 分类**：P1；API contract、状态机输入；复核历史 P1-15。
- **静态证据**：现货订单适配在 `mobile/src/api/trading.ts:174-193` 从 `Array<Record<string, unknown>>` 读取并用 `asNumber`/默认枚举；Loan 产品/订单见 `mobile/src/api/loan.ts:37-72`；Earn 见 `mobile/src/api/earn.ts:34-56,71-82`；Prediction 见 `mobile/src/api/prediction.ts:53-76,108-126`（只有 quote outcome 在 `:129-133` 严格）；New Coin 项目与记录见 `mobile/src/api/newCoin.ts:78-85,97-160,183-202`。这些必填 ID、Decimal、status 和时间字段未由生成 schema 或运行时 validator 整体验证。Margin risk 的显式 `parseMarginRiskNumber`/结构错误处理是可复用的正确方向。
- **可达影响**：字段改名、缺字段、错误类型或新枚举可能编译通过并被映射为 `0`、空字符串或默认状态；结果可能是空页面、错误按钮可用性、ID=0 请求或把未知状态显示成旧状态。服务端最终校验限制了直接资金越权，但客户端可用性和业务披露会失真。
- **增量修复**：补齐 wallet/spot/margin/seconds/convert/loan/earn/prediction/new-coin/support OpenAPI；生成 transport-only TypeScript 类型，并在响应入口对必填字段做严格 runtime decode。未知 additive 字段可忽略，未知状态必须保留为 unknown/source，而不是猜默认值。
- **兼容策略**：生成 namespace 与现有 domain model 并行；先 shadow-decode/上报但继续旧 mapper，再按领域切换。保持 URL、JSON 和页面展示 model 不变。
- **验证**：golden fixtures 覆盖缺 ID、Decimal 非数字、nullable、未知 enum、秒/毫秒时间；故意改 backend 字段时 schema freshness/contract test 必须阻断，不能渲染伪 0。
- **工作量 / 依赖**：L–XL，8–15 天，可分领域；依赖后端 OpenAPI 覆盖、代码生成与 CI schema diff。
- **运行时 caveat**：宽松解析静态成立；生产当前是否已发送畸形/新版本字段未补证。

### MCL-P2-01 — Refresh 更新 localStorage，却不更新 Pinia 会话 owner，Token 生命周期存在双事实源

- **严重级别 / 分类**：P2；auth/session 状态所有权。
- **静态证据**：Axios 通过 `mobile/src/api/client.ts:17-45` 直接读写 localStorage；成功 refresh 在 `:69-80::refreshAccessToken` 只调用 `persistAuthTokens`。Pinia 则在 `mobile/src/stores/session.ts:5-18` 独立持有 `token`，仅登录/注册等页面显式 `session.sync()`。TradeView 又同时用旧 `session.token` 作为 Margin request generation key（`mobile/src/views/TradeView.vue:187-192`），但私有 WS 的 token getter 是最新 localStorage（`:193-198`）；成功 refresh 不触发 `watch(session.token)`，因此不会主动替换 socket 或刷新 session epoch。
- **可达影响**：一次正常 401 refresh 后，请求层与页面层观察到不同 token 字符串；同一用户时通常被掩盖，但 token claims/epoch 变化、外部 storage 更新或需要主动重建私有连接时，页面 watcher 不会执行，状态失效边界难以证明。
- **增量修复**：让 Pinia session store 成为唯一 token owner，暴露 `getAccessToken/setTokens/clear/sessionEpoch`；Axios refresh 通过该 owner 原子更新，持久化只是 storage adapter。请求 stale guard 使用稳定 subject/sessionEpoch，而不是偶然的 token 文本。
- **兼容策略**：保留 `readAccessToken/persistAuthTokens` façade，内部委派 session service；先增加 token-change event，再迁移直接 localStorage 调用。
- **验证**：并发 401 只 refresh 一次；refresh 后 store、header、private URL 和 generation 同步；旧 socket 迟到事件无效；logout/refresh 竞态不能恢复旧 token；受限 storage 失败仍有明确内存状态。
- **工作量 / 依赖**：M，2–4 天；依赖可脱离 Vue 初始化的 session service 与 fake-storage tests。
- **运行时 caveat**：当前 refresh 按合同应保持同一 user subject，尚无跨用户泄漏证据；本项按结构/恢复可靠性列 P2。

### MCL-P2-02 — 私有 WS 仍是 TradeView 局部资源：未消费客服提示，也没有独立入站沉默 watchdog

- **严重级别 / 分类**：P2（客户端延迟/恢复）；复核历史 P1-10，后端跨实例可靠性仍另计 P1。
- **静态证据**：后端已向 user private channel 发送 `support.refresh`（`.trellis/spec/backend/realtime-websockets.md:86-111`、`src/modules/support/application.rs:153`）。Mobile 唯一 `createPrivateUserStream` 实例在 `mobile/src/views/TradeView.vue:193-198`，事件过滤只接受三个 Margin discriminator（`:620-627`），且仅 Contract 页面运行（`:629-637`）。SupportChat 只使用 `createSupportPollingController`（`mobile/src/views/SupportChatView.vue:135-174`），周期固定为五秒（`mobile/src/core/supportChat.ts:3,158-192`）。私有 transport 在 `mobile/src/api/privateUserStream.ts:239-276` 定时发送 ping 并忽略 pong，但没有 `lastInbound`/pong deadline/沉默关闭；退避也没有 jitter。Margin 的五秒账户 REST 对账和 Support 的五秒 REST 轮询保证正确性，是重要缓解。
- **可达影响**：客服回复即使已有服务端 hint，Mobile 仍最多等待下一轮轮询；半开私有 socket 不会自愈，Margin 低延迟提示会长期丢失。资金状态仍由 REST 收敛，因此不列 P1/P0 客户端正确性缺陷。
- **增量修复**：建立 session-scoped private connection manager + topic/handler lease；Trade 与 Support 分别租用事件，`support.refresh` 只触发序列化 REST reconcile。增加独立入站沉默 watchdog、jitter、online/visibility 恢复和 connection state；保留现有五秒轮询。
- **兼容策略**：继续暴露 `start/stop` 或 `subscribe(handler)->release` façade，逐页迁移；不得让 WS payload 成为余额/消息事实源。
- **验证**：OPEN 后静默、pong 丢失、断网恢复、token 轮换、旧 socket 迟到、最后 lease 释放；support hint 触发一次 REST，重复 hint 合并；丢掉所有 hint 后五秒 REST 仍可完整重建。
- **工作量 / 依赖**：M–L，4–7 天；依赖 MCL-P2-01 session owner、fake timer/socket harness。
- **运行时 caveat**：代理半开连接频率与跨实例命中率待网络故障注入；静态可确认的是缺 watchdog、hint 无消费者和 REST 兜底存在。

### MCL-P1-04 — Trade/Seconds/Assets 与全局样式继续承担跨域生命周期，历史巨型边界未收敛

- **严重级别 / 分类**：P1 结构债务；复核历史 P1-16，并吸收客户端 P2-03。
- **静态证据**：当前行数盘点：`mobile/src/views/TradeView.vue` 6,089 行、`SecondsView.vue` 2,818 行、`AssetsView.vue` 2,046 行、`MarketDetailView.vue` 1,502 行、`SupportChatView.vue` 1,264 行；`prototype-base.css` 8,034 行、`prototype-parity.css` 3,686 行、`pencil-selected-pages.css` 789 行，三者合计 12,509 行。TradeView 同时持有市场 REST/detail WS、shared ticker、spot/margin form、Margin setting/risk/account REST、private WS、五秒 poll、批量动作、确认弹层、路由/visibility/session ABA 和约 3,500 行模板/样式（关键生命周期集中于 `mobile/src/views/TradeView.vue:181-198,429-695,1132-1554`）。历史审计记录的 TradeView 为 5,935 行，当前并未下降。
- **可达影响**：任何视觉改动都可能触碰 socket/timer/request generation；同一文件内 spot/margin、公开/私有状态互相失效的回归面过大，review/merge 冲突和 bundle parse 成本持续上升。现有源码正则测试更容易固化结构，而不是验证生命周期行为。
- **增量修复**：按责任提取 `useMarketDetailSession`、`useMarginAccountSession`、`useFinancialMutationIntent`、workspace/dialog 组件和领域 CSS layer；页面只编排。先提 lifecycle + characterization test，再移动模板，最后拆 CSS；禁止机械按行切片。
- **兼容策略**：第一阶段保持路由、DOM data contract、API façade、Pencil geometry 和 CSS selector 输出；每次只迁移一个 owner，并保留回滚提交边界。
- **验证**：每个 composable 测 start/stop、token/mode/symbol ABA、visibility、旧响应、最后 lease、timer 清理；320/390/448 light/dark 浏览器回归；Vite manifest/chunk 与 CSS specificity budget。
- **工作量 / 依赖**：XL，分 3–6 周；依赖 MCL-P1-05 行为 harness、视觉基线和 MCL-P2-01 session owner。
- **运行时 caveat**：未执行 Vite build，故未量化压缩后 chunk、parse 或样式重算耗时；结构热点和 owner 混杂静态成立。

### MCL-P1-05 — Mobile 测试数量高但主要验证源码文本，关键点击/请求/DOM 生命周期仍没有行为门禁

- **严重级别 / 分类**：P1；测试质量、复核历史 P1-15。
- **静态证据**：静态盘点共有 80 个 `mobile/tests/*.test.ts` 文件、约 494 个 `test(...)`；其中 68 个直接读取生产源码文本。`mobile/package.json:6-13,28-42` 使用 Node test runner，依赖中没有 `@vue/test-utils`、Testing Library、jsdom/happy-dom、Playwright 或 Cypress；现有测试虽有大量纯 core/transport 单测，但没有真实 mount Vue 页面。`mobile/tests/request-layer.test.ts:30-151` 是有效 Axios 行为测试；相对地，大量页面测试以 `readFileSync` + `assert.match` 证明某段源码存在，无法证明按钮事件最终发出正确 JSON、dialog focus 或 stale response 真正被 Vue 生命周期阻断。
- **可达影响**：MCL-P1-01 这类“源码中存在函数但调用的是错误端点”、金额 round-trip、批量部分失败 toast、logout/unmount、焦点恢复和 timer 清理可在全量测试通过时仍出错。
- **增量修复**：保留源码合同用于视觉/结构防回退；增加 Vitest + Vue Test Utils/Testing Library、Axios/MSW adapter 和 fake timers。优先覆盖 wallet withdrawal/transfer、spot batch cancel、margin open/partial close, seconds open/settle、convert confirm、loan/earn/new-coin mutation、prediction quote-confirm、support send/poll。
- **兼容策略**：Node core tests 不必一次迁移；新增 `test:unit`/`test:component` 后由总 `test` 聚合，逐步把最关键 source regex 替换为行为断言。
- **验证**：故意交换按钮 handler、删除 payload 字段、让旧 promise 迟到或泄漏 timer，CI 必须失败；至少 5–10 条资金主路径做浏览器/API mock E2E。
- **工作量 / 依赖**：L，1–2 周起步；依赖稳定 fixtures、DOM runner 与 CI cache。
- **运行时 caveat**：按用户要求未运行 npm/test；数量与依赖形态来自静态文件盘点，不代表现有纯函数/transport tests 没有价值。

### MCL-P1-06 — CI 已加入 Mobile type/test，但仍不构建 PWA/Tauri；原生壳 CSP 为空

- **严重级别 / 分类**：P1 交付可靠性（CSP 子项为 P2 安全加固）；复核历史客户端构建缺口。
- **静态证据**：`scripts/p0-release-gate.sh:26-28` 对 Mobile 只运行 type-check 和 Node tests；`.github/workflows/docker-image.yml:15-56` required quality gate 仅调用该脚本，后续 Docker build 不包含 Mobile 产物。规范要求 PWA/Tauri 双构建（`.trellis/spec/mobile/index.md:15-31`），脚本本身也已有 `build:pwa`/`build:tauri`（`mobile/package.json:6-13`），但 CI 未调用。Tauri 手工构建会正确运行 `build:tauri`（`mobile/src-tauri/tauri.conf.json:6-11`），同时其 `app.security.csp` 仍为 `null`（`:24-26`），capability 仅 `core:default`（`mobile/src-tauri/capabilities/default.json:1-7`）。PWA 的 shell-only Workbox 配置本身较稳健（`mobile/vite.config.ts:40-107`），但没有产物检查进入 merge gate。
- **可达影响**：类型和源码测试通过不证明 PWA manifest/SW、Tauri publicDir 隔离、Vite chunk、CSS、资源 URL或 native bundle 可构建；发布制品可能晚于 PR 才暴露错误。CSP null 扩大任何未来 DOM/XSS 缺陷在 WebView 中的影响面，但当前未发现一条已可利用注入链。
- **增量修复**：Linux gate 先加入 `build:pwa`、`build:tauri` 和产物断言（PWA 有 manifest/SW，Tauri 无 PWA artifacts）；按平台矩阵增加 Android/iOS/desktop native compile smoke。为 Tauri 建最小 CSP allowlist，显式列出 API/WSS/Turnstile/本地资源，并做 staging rollout。
- **兼容策略**：先将构建作为 non-blocking observation 一轮，稳定后 required；CSP 用 report/log 模式盘点真实来源，再收紧，避免一次阻断 Turnstile、图片或 WS。
- **验证**：故意破坏 lazy import、PWA asset、Tauri isolation 或 CSP origin 时 gate 必须失败；检查生成 manifest/SW/precache、Tauri dist、Android/iOS debug smoke 和真实登录/行情/Turnstile。
- **工作量 / 依赖**：M–L，4–7 天；依赖 CI 时长预算、各平台 runner、签名/发布责任和 CSP 来源清单。
- **运行时 caveat**：用户明确禁止 npm/vite/PC builds，本轮未执行构建；结论是“门禁没有调用”，不是断言当前 HEAD 构建必然失败。

## Files Found

- `mobile/src/api/client.ts`、`requestAuth.ts`、`mobile/src/stores/session.ts` — Axios token refresh、localStorage 与 Pinia 会话边界。
- `mobile/src/router/index.ts`、`mobile/src/core/navigation.ts`、`mobile/src/views/LoginView.vue` — Hash 路由、懒加载、安全 redirect 和登录回跳。
- `mobile/src/api/wallet.ts`、`mobile/src/views/AssetsView.vue`、`WithdrawView.vue`、`QuickRechargeView.vue` — 钱包、提现 quote、转账和快捷充值。
- `mobile/src/api/trading.ts`、`mobile/src/views/TradeView.vue`、`OrdersView.vue` — 现货/杠杆 DTO、下单、批量操作、风险与账户对账。
- `mobile/src/api/seconds.ts`、`mobile/src/views/SecondsView.vue`、`SecondsHistoryView.vue` — 秒合约产品、订单、到期对账与历史。
- `mobile/src/api/swap.ts`、`loan.ts`、`earn.ts`、`prediction.ts`、`newCoin.ts` — 闪兑、借贷、理财、预测和新币 transport adapters。
- `mobile/src/views/SwapView.vue`、`LoanView.vue`、`EarnView.vue`、`PredictionView.vue`、`NewCoinDetailView.vue` — 二级资金业务页面与本地 mutation state。
- `mobile/src/api/privateUserStream.ts`、`marketTickerStream.ts`、`marketDetailStream.ts`、`webSocketLiveness.ts` — 私有/公开市场 socket 生命周期。
- `mobile/src/api/support.ts`、`mobile/src/core/supportChat.ts`、`mobile/src/views/SupportChatView.vue` — 客服 REST、幂等发送、游标分页和轮询。
- `mobile/src/styles/prototype-base.css`、`prototype-parity.css`、`pencil-selected-pages.css` — 全局/共享视觉热点。
- `mobile/tests/*.test.ts`、`mobile/package.json` — Node 测试入口、源码合同与缺失的 Vue DOM harness。
- `mobile/vite.config.ts`、`mobile/src-tauri/tauri.conf.json`、`mobile/src-tauri/capabilities/default.json` — PWA/Tauri 构建隔离与原生安全配置。
- `scripts/p0-release-gate.sh`、`.github/workflows/docker-image.yml` — 当前 required CI 边界。
- `src/modules/spot/routes.rs`、`src/modules/spot/application/cancellation.rs` — 现货批量撤单权威端点与部分失败结果。
- `src/modules/support/application.rs`、`.trellis/spec/backend/realtime-websockets.md` — `support.refresh` 私有提示生产端合同。

## Code Patterns

- **正确模式**：WS 只作 refresh hint，Margin/Support 周期 REST 仍是权威；请求使用 generation/当前 socket identity 防迟到结果。
- **仍漂移的模式**：HTML string input → `Number(...)` → domain input `number` → `String(number)` → Rust `BigDecimal`。
- **宽松 transport 模式**：`Record<string, unknown>` → `asNumber(..., 0)`/空字符串/默认枚举 → UI 继续运行，而不是在边界报告 contract error。
- **局部 owner 模式**：复杂 lifecycle 在 TradeView/SecondsView 内成熟，普通二级资金页各自维护 loading/error/mutation，缺统一 session request owner。
- **测试模式**：纯函数/transport 行为测试与大量源码正则并存；后者能锁结构，却不能代替 Vue DOM 与请求行为测试。
- **交付模式**：PWA/Tauri 手工脚本合同完整度高于 required CI 实际调用范围。

## External References / Versions

- 未联网检索外部资料；全部结论来自 2026-08-30 当前工作树、Trellis specs、历史审计和本轮静态命令。
- `mobile/package.json`：Vue 3.4、Pinia 2.1、Axios 1.6、Vue Router 4.3、Vite 5.2、vite-plugin-pwa 1.3、Tauri 2.9、lightweight-charts 5.2.0。
- 后端金额与实时合同版本以仓库内 `.trellis/spec/backend/` 为准；未假设生产部署已同步最新二进制或 migration。

## Related Specs

- `.trellis/spec/mobile/index.md` — Mobile 质量检查与 PWA/Tauri 双构建入口。
- `.trellis/spec/mobile/backend-integration.md` — runtime URL、auth refresh、公开/私有 WS、金融 DTO、Margin/Support REST 对账。
- `.trellis/spec/mobile/navigation-and-localization.md` — Hash history、安全 redirect、返回与 i18n。
- `.trellis/spec/mobile/pwa-and-shell.md` — PWA shell-only cache、Tauri isolation、可访问性/视觉与巨型页面行为合同。
- `.trellis/spec/backend/auth-sessions.md` — user token/refresh/private WS 会话合同。
- `.trellis/spec/backend/realtime-websockets.md` — 公开行情活性、Margin/Support 私有提示与 REST 恢复。
- `.trellis/spec/backend/wallet-amount-precision.md` — 0..18 scale、Decimal 与费用精度。
- `.trellis/spec/backend/spot-orders.md`、`margin-trading-actions.md`、`seconds-contracts.md` — 现货批量、杠杆能力/部分失败、秒合约价格/精度。
- `.trellis/spec/backend/loan-products.md`、`earn-products.md`、`prediction-markets.md`、`new-coin-mobile-contract.md`、`online-support.md` — 二级产品与客服权威合同。
- `.trellis/spec/guides/cross-layer-thinking-guide.md`、`code-reuse-thinking-guide.md` — 跨层数据流和 lifecycle/transport 复用边界。

## Caveats / Not Found

- 严格按用户边界只做静态读取并只写本文件；未修改生产代码、spec、进度、任务元数据或其他 research 文件，也未执行任何 git 操作。
- 按用户要求未运行 npm、Vite、PWA/Tauri、PC 或 Rust builds/tests；没有连接浏览器、Tauri 原生壳、MySQL、Redis、RabbitMQ、生产 API 或真实网络故障注入。
- 行号基于 2026-08-30 当前工作树；同时给出符号名，后续未提交 Mobile 改动可能移动行号。
- 未实测 bundle/chunk、CSS style-recalc、WebView CSP、半开 socket、跨实例广播、生产资产精度和真实用户数据分布；相应影响均在条目中明确保留 runtime caveat。
- 未发现足够证据单列新的 route/navigation、直接越权、客户端可信价格动账或 P0 资金错误；“未发现”只限本次有界静态样本，不等于生产运行时证明。
