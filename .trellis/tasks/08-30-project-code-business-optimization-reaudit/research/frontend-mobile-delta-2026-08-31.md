# Research: Mobile 前端 CURRENT HEAD 增量复审（2026-08-31）

- Query: 以 2026-08-30 `mobile-cross-layer.md` 为基线，仅复审当前 Mobile 前端（`mobile/src`、`mobile/tests`、Mobile package/Vite/PWA/Tauri 与直接发布门禁配置），识别 routing、API/request lifecycle/cache、state ownership、WebSocket liveness/reconciliation、Decimal、i18n、accessibility/dialog/focus、performance、巨型组件/样式、测试质量和 PWA/Tauri 交付方面仍存在或新识别的问题。
- Scope: internal（当前检出内容的静态审计 + 无生产写入的本地验证；未访问生产 API/WS，未修改生产代码、规范或既有研究）
- Date: 2026-08-31

## Findings

### 1. 口径、delta 定义与摘要

- 优先级沿用任务 PRD：P0 仅用于可直接影响资金、权限、结算、价格时点或不可恢复数据正确性的风险；P1 用于业务可用性、跨进程可靠性和显著维护/交付风险；P2 用于体验、性能、一致性、无障碍及长期治理（`.trellis/tasks/08-30-project-code-business-optimization-reaudit/prd.md:15-20`）。
- **延续**：2026-08-30 已记录且当前源码仍能证明；**新增识别**：本具体缺口未在 8 月 30 日 Mobile 条目中展开，不代表已通过提交历史证明它在 8 月 30 日后引入；**已验证改善/未发现**：当前检查未形成新的缺陷条目。
- 证据标签：
  - **确认的静态缺陷**：当前类型、控制流、模板或配置已足以证明缺口。
  - **本地确定性复现**：本轮用无写入 mock/纯函数调用直接复现结果。
  - **运行时假设**：生产触发频率、真实网络/浏览器行为、实际资产精度、制品构建或平台体验仍需后端 fixture、浏览器、故障注入或产物检查确认。
- 本轮保留/新增 **0 项 P0、7 项 P1、5 项 P2**。最高新增风险是：冷启动行情 refresh 的非共享 Promise 可在路由切换时留下“有 REST 快照但无实时流”的页面；认证 refresh 可在用户 logout 后重新写回 token 并重放请求；新增 Wallet Ledger 把 9–18 位小数和大额 Decimal 读模型压成 `number`/最多八位展示。

### 2. 2026-08-30 Mobile 条目 delta 映射

| 2026-08-30 条目 | 当前状态 | 本轮定位 |
|---|---|---|
| MCL-P1-01 现货“全部撤单”逐笔模拟 | **延续** | FMD-P1-04 |
| MCL-P1-02 mutation Decimal 经 `number` | **延续，并新增 Ledger 读模型精度复现** | FMD-P1-03 |
| MCL-P1-03 宽松手写 DTO | **延续，未重复计数** | Decimal 与未知 status 的具体后果分别并入 FMD-P1-03、FMD-P2-02 |
| MCL-P2-01 localStorage/Pinia 双 owner | **延续；新增 logout-during-refresh 确定性复现，升为 P1** | FMD-P1-02 |
| MCL-P2-02 私有 WS 局部持有、无沉默 watchdog | **延续** | FMD-P2-05 |
| MCL-P1-04 巨型视图/样式 | **延续且恶化** | FMD-P1-05 |
| MCL-P1-05 源码正则测试主导 | **延续；测试数量增长但行为门禁未变** | FMD-P1-06 |
| MCL-P1-06 CI 不构建 PWA/Tauri、CSP 为空 | **延续；新增 PWA 更新 busy 恢复缺口** | FMD-P1-07 |
| 路由与无障碍未单列缺陷 | **功能路由仍未发现新断裂；新增 route focus/landmark/listbox 证据** | FMD-P2-03 |

---

### FMD-P1-01 — 冷启动行情 refresh 不共享 in-flight，路由切换可永久漏启共享 ticker 流

- **Delta / 优先级**：新增识别；P1（行情实时可用性与参考价格新鲜度）。
- **证据类型**：请求/租约竞态是**确认的静态缺陷**；生产中命中该时序的频率、静止价格持续时长及下单服务端是否完全消除影响是**运行时假设**。
- **代码证据**：
  - Pinia `refresh()` 在 `loading` 为真时直接返回一个已完成的 `Promise<void>`，没有返回或 join 首个请求的 Promise（`mobile/src/stores/market.ts:17-30::refresh`）。第二个调用者的 `await` 因而不等待 tickers 被填充。
  - `startLiveUpdates()` 先登记 consumer，但在 `tickers` 为空时直接返回；之后没有 tickers watcher、refresh completion hook 或 `ensureLive()` 再次启动连接（`mobile/src/stores/market.ts:33-54::startLiveUpdates`）。
  - 四个页面都采用“`await refresh()` 后若仍 active 才 start”的同型顺序：Home（`mobile/src/views/HomeView.vue:339-351`）、Markets（`mobile/src/views/MarketsView.vue:140-152`）、Trade（`mobile/src/views/TradeView.vue:1484-1490,1575-1584`）、Market Detail（`mobile/src/views/MarketDetailView.vue:455-467`）。
  - 可达时序：A 页面开始首次 refresh，网络未返回；A 卸载，B 页面挂载并调用 refresh；B 因 `loading=true` 立即返回，随后把 consumer 加入但因 tickers 为空不建 socket；首个请求完成后 A 的 continuation 因 `viewActive=false` 跳过 start。此时 tickers 已有 REST 数据、B consumer 仍在，但 `stopLive=null`，且没有任何自动重试。下一次显式 refresh/start 或再次导航前，页面可一直展示静止快照。
  - 本地规范要求每个页面在初始 refresh 后持有稳定 lease，离开页不得关闭进入页的 lease（`.trellis/spec/mobile/backend-integration.md:186-193`）；当前实现只覆盖已成功走到 `startLiveUpdates` 的调用者。
  - `mobile/tests/market-price-authority.test.ts:109-127` 仅以 `readFileSync`/正则证明语句和 Set 存在；所谓“route transition consumers overlap”测试没有 deferred refresh、Pinia store 或 socket 行为，因此不会捕获上述 cold-start interleaving。
- **影响**：价格、24h 高低/成交量/涨跌幅停止更新，但 `updatedAt` 刚被 REST 写成当前时间且 UI 没有“live stream 未启动”状态，用户无法区分实时行情与静止快照。交易最终结算是否受影响依赖服务端权威校验；客户端参考价/市场判断失鲜静态可达。
- **整改**：
  1. Store 保存 `refreshPromise`，所有并发调用 join 同一 Promise；只允许完成者清理相同 promise identity。
  2. 把 `liveConsumers` 作为期望状态，抽出 `ensureLive()`；refresh 成功、consumer 新增、socket 结束后都以“`consumers.size > 0 && tickers.length > 0 && !stopLive`”幂等确保连接。
  3. consumer ID 继续保持兼容 façade，但页面不再承担“数据是否已到齐”的启动责任；store 暴露 `live/connecting/stale/lastFrameAt` 供 UI 与故障诊断。
  4. refresh 失败保留可重试错误，不能把 consumer 永久留在无连接、无原因状态。
- **验证**：用 fake fetch/socket + deferred promises 覆盖 A refresh→A unmount→B mount→B join→首请求完成；断言只发一组 REST、恰好一条 socket、B lease 仍有效。补充双页面并发、首请求失败后 retry、最后 lease 释放、重复 start/stop、A→B→A 和无 tickers 响应。应将测试放在 store 行为层，而不是源码正则层。
- **工作量 / 依赖**：S–M，1–2 天；依赖可注入的 market fetch/socket factory 或 Pinia 行为测试 harness。

### FMD-P1-02 — Refresh 缺 session epoch/CAS，logout 后可复活 token 并重放旧请求

- **Delta / 优先级**：延续并细化 MCL-P2-01；P1（会话完整性）。若补证“旧账号 refresh 覆盖新账号登录”可达，应按权限边界重新评估 P0。
- **证据类型**：双 owner 与竞态控制流是**确认的静态缺陷**；本轮已用 Axios mock 做**本地确定性复现**。真实跨账号结果仍需后端 token fixture。
- **代码证据**：
  - Axios 从 localStorage 直接读取 token，`persistAuthTokens()` 直接写两个独立 key；写入没有 catch、rollback 或原子 envelope（`mobile/src/api/client.ts:17-45`）。refresh 在发请求前只快照 refresh token，响应后不验证当前会话/epoch，直接持久化新 token（`:69-80::refreshAccessToken`）。
  - response interceptor 全局共享 refresh Promise，await 后只要得到 token 就给原 protected request 写 Bearer 并重放；期间没有 logout generation、原 refresh token 比对或 session-active 检查（`mobile/src/api/requestAuth.ts:30-38,53-73`）。
  - Pinia store 独立持有初始化时读取的 token；logout 仅清 storage 和本地 ref（`mobile/src/stores/session.ts:5-18`）。成功 refresh 不调用 `session.sync()`，因此 App 的 token watcher也不会运行（`mobile/src/App.vue:49-60`）。
  - 登录/注册/2FA/password change 同样先在 API 模块持久化，再由个别页面显式 `session.sync()`（`mobile/src/api/auth.ts:84-104,137-147`、`mobile/src/views/LoginView.vue:108-112`、`mobile/src/api/user.ts:236-242`），证明会话没有单一 owner。
  - 本轮 mock 时序为：旧 protected request 收到 401→refresh pending→将 storage/store token 清空模拟 logout→refresh 返回 `ACCESS_NEW`。结果原请求被第二次发送并返回 200，storage 为 `ACCESS_NEW`、Pinia 模拟值仍为空，`splitBrain=true`。现有 `mobile/tests/request-layer.test.ts:62-98` 只覆盖并发 401 正常共享；`:124-151` 只覆盖 refresh 失败，没有 logout/login 与 pending refresh 的交叉测试。
  - 次级 cache hygiene：钱包参考目录直接把完整 bearer token 放进 process-memory key（`mobile/src/api/wallet.ts:306-309`），registry 的 cache/in-flight Map 只在显式 invalidation 或再次命中相同 key 时回收（`mobile/src/api/requestCache.ts:23-79`），而 logout/refresh 没有调用 registry invalidation。当前 key 隔离可阻止另一个 token直接命中旧数据，因此本轮不把它计为跨账号缓存泄漏；但旧 token 和目录对象会滞留到页面进程结束。
- **影响**：用户执行 logout 后，稍后完成的 refresh 可重新写 token，旧请求继续成功；当前页面可能仍显示 guest，但 reload 后重新变成登录态。更危险的待补证时序是 logout→登录账号 B→旧账号 A refresh 最后返回，storage 与 Pinia/页面会话可能分别指向不同账号，后续 Axios 和页面 stale guard 使用不同身份。
- **整改**：
  1. 建立脱离 Vue 初始化也可访问的单一 Session service/Pinia owner，原子持有 `{accessToken, refreshToken, subject, epoch}`；storage 只是 adapter。
  2. refresh 捕获 `{epoch, refreshToken}`，完成时做 compare-and-swap；logout、登录新账号、密码换 token 都递增 epoch 并使旧 refresh 结果与 replay 失效。
  3. interceptor replay 前再次验证 request session 与当前 epoch；旧请求返回 typed `stale/session-ended`，不得触发全局重新登录。
  4. 用一个 JSON envelope 或带 rollback 的 storage adapter 避免 access/refresh 两 key 半写入；受限 storage 时保持内存会话可用并显式暴露 persistence failure。
  5. session transition 清理/失效所有 private request、WS lease 与账号作用域 cache；cache key 使用不可逆 session scope ID，而非原 bearer 文本。
- **验证**：deferred refresh 覆盖 refresh→logout、refresh→logout→login B、两次 refresh、password token rotation、storage.setItem 第一次/第二次抛错、跨 tab storage event；断言旧结果不持久化、不 replay、不恢复 socket/cache，store/storage/Authorization 始终同一 subject/epoch。
- **工作量 / 依赖**：M，3–5 天；依赖 session service、可注入 storage 和请求/WS/cache transition hooks。

### FMD-P1-03 — Decimal mutation 缺口延续；新增 Wallet Ledger 把合法 18 位金额显示成 0 或另一余额

- **Delta / 优先级**：延续 MCL-P1-02，并新增 Wallet Ledger 精度证据；P1（资金输入与审计显示正确性）。
- **证据类型**：转换和显示损失是**确认的静态缺陷**，Ledger 已做**本地确定性复现**；生产是否开放 9–18 位 scale 资产、实际金额分布仍是**运行时假设**。
- **代码证据 — 写边界仍未修复**：
  - 现货 `SpotOrderInput.price/quantity`、杠杆 `marginAmount` 仍为 number，再通过 `String(...)` 组 payload（`mobile/src/api/trading.ts:27-43,174-190,275-290`）。
  - Seconds stake（`mobile/src/api/seconds.ts:63-77`）、Convert amount（`mobile/src/api/swap.ts:48-53`）、Loan amount/collateral（`mobile/src/api/loan.ts:86-92`）、Earn amount（`mobile/src/api/earn.ts:70-74`）、Prediction stake（`mobile/src/api/prediction.ts:87-93`）、New Coin quote/quantity/fee（`mobile/src/api/newCoin.ts:96-101,171-183`）和 Wallet 快捷充值/划转（`mobile/src/api/wallet.ts:548-582`）均在 string 化前已经是 IEEE-754 number；New Coin 还以浮点 `quoteAmount / issuePrice` 派生 quantity。
  - 这与 Mobile mutation 边界要求 Decimal string（`.trellis/spec/mobile/backend-integration.md:323-328`）和资产 `precision_scale=0..=18`、存储 `DECIMAL(38,18)` 的后端契约（`.trellis/spec/backend/wallet-amount-precision.md:12-22`）冲突。
- **代码证据 — Ledger 新增具体后果**：
  - Ledger 类型把 `amount/fee/balanceAfter` 定义为 number，并固定 `WALLET_LEDGER_MAX_FRACTION_DIGITS=8`（`mobile/src/core/walletLedger.ts:20-23,54-64`）。
  - mapper 通过 `requiredRealizedReturnNumber` 转换三个 Decimal 字段（`mobile/src/core/walletLedger.ts:428-479`），而该 helper 最终执行 `Number(value.trim())`（`mobile/src/core/realizedReturn.ts:4-20`）。
  - formatter 最多展示八位小数（`mobile/src/core/walletLedger.ts:362-369`）；页面把该 formatter用于金额、余额和手续费（`mobile/src/views/WalletLedgerView.vue:147-159,279-286`）。
  - 测试将 `0.00000001` 称为“最小非零单位”，并只锁定八位（`mobile/tests/wallet-ledger-classification.test.ts:335-342`）。Mobile spec 也新写成最多八位（`.trellis/spec/mobile/backend-integration.md:1013-1016`），与项目资产 0..18 scale 规范自相矛盾。
  - 本轮直接调用 mapper/formatter：`amount='0.000000001000000000'` 映为 `1e-9` 后显示 `0`；`fee='0.000000000000000001'` 显示 `0`；`balance_after='9007199254740993.000000000000000001'` 映为并显示 `9,007,199,254,740,994`，已不是源余额。
- **影响**：mutation 可能发送不同于用户输入的合法 Decimal；Ledger 又可能把真实非零资金变动/手续费呈现为 0，或把大额余额改成另一整数，破坏用户对账与客服审计。当前是否已有受影响资产不能仅由 Mobile 静态代码确认。
- **整改**：
  1. 引入 branded `DecimalText` transport/domain 类型；输入 ref、确认快照、幂等 intent 与 JSON payload 全程保留规范化字符串。
  2. 数学运算使用 Decimal 库并按资产 `precision_scale` 明确截断；禁止 `String(number)` 作为资金边界修复。
  3. Ledger adapter 保留 amount/fee/balance 的源 Decimal text 和资产 scale；比较/符号用 Decimal，formatter 至少覆盖该资产 scale，任何合法非零值不得显示为 0。
  4. 通过 `trellis-update-spec` 单独修正 Mobile Ledger 的“固定八位”规范；本研究代理不修改规范。
- **兼容策略**：API façade 可短期接收 `string | number`，入口立即拒绝不安全 number 或记录迁移 telemetry；按 wallet/spot→margin/seconds→convert/loan/earn/prediction/new-coin 分批收口，不一次性重写页面。
- **验证**：scale 0/2/8/9/18、`0.000000000000000001`、尾零、负数、超过 `2^53`、最大 DECIMAL(38,18)、fee/tier 边界；断言输入→确认→幂等 intent→请求 JSON→Ledger 显示逐字符/明确量化一致。加入当前三条本地复现 fixture，旧实现必须失败。
- **工作量 / 依赖**：L，6–12 天；依赖资产 precision 元数据、Decimal 库、DTO 收口和规范更新。

### FMD-P1-04 — 现货“全部撤单”仍是 N 次 DELETE，部分成功结果被压成单一失败

- **Delta / 优先级**：延续 MCL-P1-01；P1。
- **证据类型**：**确认的静态契约缺陷**；生产部分失败频率和实际返回文案未发请求验证。
- **代码证据**：
  - Orders 页面把当前列表全部 ID 交给 adapter，并在任一异常时只显示统一失败（`mobile/src/views/OrdersView.vue:261-275::cancelAllSpot`）。
  - adapter 明写“后端暂未提供”，对每个 ID 调单笔 `DELETE /spot/orders/{id}`，`Promise.allSettled` 后只抛第一个 rejected，不返回成功数、失败 ID/code/message（`mobile/src/api/trading.ts:217-235`）。
  - 权威规范已有单次 `DELETE /api/v1/spot/orders?pair_id=...`，响应分别包含 `orders[]` 与 `failures[]`，且部分失败不得阻断后续项（`.trellis/spec/backend/spot-orders.md:74-116`）。
  - 三组页面测试只用源码正则断言 `cancelAllSpotOrders(...)` 被调用（如 `mobile/tests/secondary-product-order-views.test.ts:28-45`），不会验证端点、调用次数或部分失败 UI。
- **影响**：大量委托产生 N 次网络/事务开销；连接波动时一部分订单已撤、一部分仍开，但 UI 只有整体失败，用户无法知道剩余风险敞口。随后 `load()` 可能刷新列表，但反馈仍不表达 server failures 的原因和数量。
- **整改**：调用服务端批量端点，定义严格 `SpotCancelAllResult { orders, failures }`；按响应立即 reconcile 当前列表并显示“成功 N / 失败 M”，失败项保留 ID、可重试状态与服务端 message。可选 pair scope 必须与 UI 文案一致。
- **验证**：全部成功、空集合、1 成功/1 失败、legacy malformed、重复请求、pair filter、响应丢失后重取；行为测试断言只发一个 DELETE、payload/query 正确、部分失败不显示全成功且列表与服务端结果一致。
- **工作量 / 依赖**：S，0.5–1 天；无新后端能力依赖。

### FMD-P1-05 — 巨型视图/样式继续增长，Seconds 一天增加 577 行且缺 bundle/CSS budget

- **Delta / 优先级**：延续 MCL-P1-04，规模继续恶化；P1 结构/性能风险。
- **证据类型**：文件规模、owner 混杂和重复样式是**确认的静态风险**；压缩 chunk、parse/style-recalc、低端机帧时与内存尚属**运行时待测**。
- **当前量化**：

| 热点 | 2026-08-30 | 2026-08-31 | Delta |
|---|---:|---:|---:|
| `mobile/src/views/TradeView.vue` | 6,089 | 6,125 | +36 |
| `mobile/src/views/SecondsView.vue` | 2,818 | 3,395 | +577 |
| `mobile/src/views/AssetsView.vue` | 2,046 | 2,086 | +40 |
| `mobile/src/views/MarketDetailView.vue` | 1,502 | 1,502 | 0 |
| `mobile/src/views/SupportChatView.vue` | 1,264 | 1,264 | 0 |
| 三个共享样式文件合计 | 12,509 | 12,609 | +100 |

  - 当前 `prototype-base.css` 8,034 行、`prototype-parity.css` 3,686 行、`pencil-selected-pages.css` 889 行；无 CSS/chunk budget。
  - Trade 约 3,357 行 style、Seconds 约 1,838 行、Assets 约 1,204 行；新增/扩大的 `mobile/src/components/ContractTradeSheets.vue` 仍有 1,501 行，其中约 916 行 style。拆出文件没有形成小责任边界。
  - Mobile source 当前 54 个 Vue 文件约 1.17 MB、84 个 TS 文件约 0.53 MB、5 个 CSS 文件约 0.27 MB。全树仍有 16 个局部 `@keyframes spin`，同时 `mobile/src/styles/base.css` 又定义全局 functional spinner；这至少形成重复 CSS 和命名碰撞审查面。
  - TradeView 继续同时持有市场 REST/detail WS/shared ticker、spot/margin mutation、private WS、五秒 REST 对账、visibility/session guard、弹层和大段样式；FMD-P1-01/02/03 的生命周期、会话和 Decimal 风险都在这个边界相遇。
- **影响**：review 与 merge 冲突面持续扩大；视觉改动可意外影响 request/socket/timer owner；CSS 重复和超长 SFC 增加编译、解析与低端 WebView 样式审查成本。源码正则测试更倾向固化 DOM/CSS 形状，难以保护真实行为。
- **整改**：按 owner 提取 `useMarketSession`、`useFinancialMutationIntent`、`useSecondsOrderSession`、dialog/picker components 和领域 CSS layer；先建立 characterization/behavior tests，再一次迁移一个 owner，保持路由、API façade 与 data/Pencil selectors。对 SFC/style/chunk 建预算和趋势门禁，不按机械行数一次性切文件。
- **验证**：composable 覆盖 start/stop、token/mode/symbol ABA、visibility、旧响应、timer 清理；320/390/448、light/dark、reduced-motion 浏览器回归；PWA/Tauri manifest 中记录 raw/gzip/brotli JS/CSS、chunk count、long-task 与首屏内存基线。
- **工作量 / 依赖**：XL，分 3–6 周；依赖 FMD-P1-06 的组件/浏览器 harness 与视觉基线。

### FMD-P1-06 — 90 个测试仍由源码读取主导，538/538 通过也未阻断本轮竞态和精度缺陷

- **Delta / 优先级**：延续 MCL-P1-05；P1 测试治理。相较 8 月 30 日，测试文件 80→90、实际执行 test 494→538、源码读取文件 68→76；数量增长没有改变主要测试形态。
- **证据类型**：**确认的静态/本地验证缺陷**。
- **代码证据与验证**：
  - 当前 90 个 `mobile/tests/*.test.ts` 中 76 个调用 `readFile/readFileSync`，占 84.4%。`mobile/package.json:6-21` 只有 Node test、type-check 和 build scripts；依赖中没有 Vue Test Utils/Testing Library、jsdom/happy-dom、Vitest、Playwright/Cypress，也没有 lint、coverage 或 e2e script。
  - `mobile/tsconfig.json:21-22` 明确 exclude `tests` 和 `src-tauri`，因此 `npm run type-check` 不检查测试 TypeScript，也不编译 Rust/native 配置。
  - `mobile/tests/market-price-authority.test.ts:120-127` 用正则宣称 lease 能覆盖路由重叠，却不执行 refresh/lease interleaving；`mobile/tests/seconds-pair-picker-pencil-parity.test.ts:31-66` 只验证 `role=option` 和 handler 文本存在，不执行 listbox 键盘；`mobile/tests/pwa-status-immersive.test.ts:7-31` 只检查 update action 名称和模板结构，不执行 service-worker 状态机。
  - `mobile/tests/functional-spinner.test.ts:5-18` 只读取 `base.css` 就声明全局 spinner 约束；全 source 实际还有 16 个局部 `@keyframes spin`，说明该测试无法作为全树重复样式门禁。
  - 本轮 `npm --prefix mobile test` 为 **538/538 PASS**，但 FMD-P1-01、FMD-P1-02、FMD-P2-01、FMD-P2-03 和 PWA busy 缺口仍可由控制流证明；这不是测试失败，而是覆盖边界不足。
  - 正面证据：`request-cache.test.ts`、`market-ticker-stream.test.ts`、`market-detail-stream.test.ts`、`margin-account-reconciliation.test.ts`、`wallet-ledger-classification.test.ts` 中存在有价值的纯函数/fake timer/socket/lifecycle 行为测试；应保留并扩展，而非全量迁移源码合同。
- **影响**：按钮 handler、payload Decimal、logout/refresh、路由焦点、deferred response、service-worker 更新、Blob URL 和真实 DOM role/focus 可错误但全量 CI 仍绿。测试自身类型漂移也不会被 vue-tsc 阻断。
- **整改**：新增测试 tsconfig 与统一 `test:unit/test:component/test:e2e/test`；以 Vitest + Vue Test Utils/Testing Library（或等价方案）、Axios/MSW、fake timers/service worker、Playwright 建行为层。优先把资金 mutation、session refresh/logout、market cold start、Orders tab race、dialog/listbox/focus 和 PWA update 转成 deferred-promise/DOM 测试；源码测试仅保留必要视觉/构建合同。
- **验证**：故意交换 tab response、在 logout 后 resolve refresh、删除 Arrow 键逻辑、让 SW 不发 controllerchange、把 1e-9 显示为 0，CI 必须分别失败；tests 纳入 type-check，核心资金/session/realtime modules 设 branch coverage 与 mutation testing 样例。
- **工作量 / 依赖**：L，1–2 周建立基础，随后逐域迁移；依赖 CI 浏览器资源与稳定 fixtures。

### FMD-P1-07 — 发布门禁仍不构建 PWA/Tauri，Tauri CSP 为空；PWA 更新确认后无失败恢复

- **Delta / 优先级**：延续 MCL-P1-06；总体 P1 交付可靠性。CSP 与 PWA update recovery 子项按 P2 安全/体验处理。
- **证据类型**：CI/配置缺口是**确认的静态缺陷**；当前 HEAD 是否能成功构建、具体 WebView/CSP 来源及 SW 卡住频率是**运行时假设**。
- **代码证据**：
  - Mobile 已定义 `build:pwa` 与 `build:tauri`（`mobile/package.json:6-21`），Tauri beforeBuild 也调用 `build:tauri`（`mobile/src-tauri/tauri.conf.json:6-11`）；但 required release gate 对 Mobile 只执行 type-check 和 Node tests（`scripts/p0-release-gate.sh:33-35`），`.github/workflows/docker-image.yml:61` 仅调用该 gate。规范要求 PWA/Tauri 双构建和产物检查（`.trellis/spec/mobile/index.md:15-31`、`.trellis/spec/mobile/pwa-and-shell.md:778-793`）。
  - Tauri `app.security.csp` 仍为 `null`（`mobile/src-tauri/tauri.conf.json:24-26`）；capability 目前仅 `core:default`（`mobile/src-tauri/capabilities/default.json:1-7`）。当前未找到可利用的 DOM 注入链，因此 CSP 不单独升级。
  - PWA shell 配置本身保持 `generateSW`、prompt update、`runtimeCaching: []`、API/WS fallback denylist 和 Tauri isolation（`mobile/vite.config.ts:40-107`），这是正确方向，但只有源码测试，没有 generated manifest/SW/precache/Tauri dist 的 required artifact assertion。
  - 新增 recovery 缺口：`applyPwaUpdate()` 将 `state.updating=true`，找到 waiting worker 后发送 `SKIP_WAITING` 并直接返回 true（`mobile/src/pwa/index.ts:237-255`）；只有后续 `controllerchange` 会 reload（`:161-184`），没有 timeout、worker state error、postMessage catch 或恢复 `updating=false` 的路径。若 controllerchange 不到达，PWA 卡片持续 `aria-busy`，Update/Later 都被 disabled（`mobile/src/components/PwaStatus.vue:91-115`）。触发本身需真实浏览器/SW 故障注入，故归为运行时假设；“没有恢复分支”静态成立。
- **影响**：类型和源码测试通过不证明 PWA/Tauri 可产出、PWA/Tauri 资源隔离、manifest/SW、asset URL 或 native bundle 正确。CSP null 扩大未来 renderer 注入影响。SW 激活失败时用户只能保留永久 busy 的更新卡片，直到 reload/进程重启。
- **整改**：
  1. Linux gate 加 `build:pwa`、`build:tauri` 与 artifact assertions；平台矩阵增加 Android/iOS/desktop compile smoke，先 observation 后 required。
  2. PWA update 使用可测试状态机：等待 controllerchange/worker redundant/error，设有界 timeout，失败时清 busy、显示本地化 retry，并确保重复点击幂等。
  3. 建最小 Tauri CSP allowlist，显式覆盖 API/WSS/Turnstile/本地资源；先 staging telemetry，再逐步收紧。
  4. 若产品要求原生 updater，再单独定义签名、channel、rollback 与 capability；当前 Mobile 配置中未找到 updater，需求本身未在本轮 scope 证明，故不作为现有缺陷计数。
- **验证**：PWA build 检查 manifest/SW/precache 且无 API/WS runtime cache；Tauri web dist 断言无 PWA artifacts。fake SW 覆盖 waiting→controllerchange、waiting 消失、redundant、postMessage throw、timeout、重复 apply；真实 Chrome installed PWA 和 Tauri staging smoke。CSP deny/allow 覆盖登录、Turnstile、图片、HTTP 与 WS。
- **工作量 / 依赖**：M–L，4–7 天；依赖 CI runner、平台 SDK、CSP 来源清单和发布责任人。

---

### FMD-P2-01 — OrdersView 没有 request generation，旧 tab 请求可覆盖当前 loading/error

- **Delta / 优先级**：新增识别；P2（请求生命周期与状态一致性）。
- **证据类型**：并发写入是**确认的静态缺陷**；用户点击/网络延迟组合的发生率需组件运行时补证。
- **代码证据**：
  - `load()` 每次直接设置共享 `loading/feedback/error`，读取可变 `marketTab/stateTab` 后 await 不同请求，完成时无 request version、tab/session snapshot、AbortSignal 或 mounted guard；任何旧请求的 catch/finally 都可写当前 `error/loading`（`mobile/src/views/OrdersView.vue:206-243::load`）。
  - 同一 tab 再点击会直接再发 `load()`（`:122-137`）；watcher 在 tab 变化时再发（`:389`）；mounted 根据 route query 修改一个或两个 refs后又手动 `load()`（`:405-415`），可产生重复或并行调用。
  - `onBeforeUnmount` 只恢复 body overflow，没有停止/失效请求（`:417-419`）。
  - 可达例：margin history 慢请求开始→切到 spot/current→spot 成功并显示→旧 margin 失败，最终把 spot 页面 `error` 写成 margin 错误；或旧请求先 finally 将 `loading=false`，而最新请求仍在飞。数组按领域分开减少了错误列表覆盖，但共享状态仍不属于一个 request key。
- **影响**：当前页可能提前去掉 spinner、显示另一个 tab 的错误或重复请求；在取消/平仓后的 refresh 与手工 tab 切换叠加时，成功/失败反馈也可能被旧 load 清空。
- **整改**：复用/扩展 `mobile/src/core/sessionRequest.ts:13-47::createSessionRequestLifecycle`，request key 至少包含 `{sessionEpoch, marketTab, stateTab}`；只允许当前 key/version提交 data/error/loading。route query 初始化应原子设置后仅触发一次 load；unmount/logout 立即 invalidate，必要时 AbortController 取消网络。
- **验证**：组件测试用 deferred promises 覆盖 margin→spot、history→current、同 tab double refresh、mounted query 两 ref、mutation refresh 与 unmount/logout；让旧请求最后 success/fail，断言当前 DOM/data/error/loading 只属于最新 key。
- **工作量 / 依赖**：S–M，1–2 天；依赖 FMD-P1-06 组件 harness。

### FMD-P2-02 — i18n key 对称但业务 status/category 仍直接泄漏，KYC 未知状态被伪装为 pending

- **Delta / 优先级**：新增识别；P2 国际化与状态真实性。
- **证据类型**：**确认的静态缺陷**。
- **代码证据**：
  - 本轮递归盘点 `zh-CN`/`en` 各 1,744 个 leaf key，缺失 key 0、placeholder mismatch 0；扫描 1,822 个 literal `t/$t/.t` 调用，缺 key 0。基础 key 完整性是健康项，但 `mobile/tests/i18n.test.ts:1-18` 本身只测 locale normalization/API locale，并未建立全局 parity 门禁。
  - 已确认直接呈现 backend machine enum：邀请 `invite.status`（`mobile/src/views/ReferralsView.vue:133-142`）、快捷充值 `order.status`（`mobile/src/views/QuickRechargeView.vue:165-174`）、闪兑 `order.status`（`mobile/src/views/SwapView.vue:306-315`）、理财 `subscription.status`（`mobile/src/views/EarnView.vue:283-296`）；Earn 产品 category 也直接输出（`:262-274`）。
  - API adapter 保留原始 status：Referral（`mobile/src/api/user.ts:260-278`）、Quick Recharge（`mobile/src/api/wallet.ts:608-623`）、Swap（`mobile/src/api/swap.ts:75-90`）、Earn（`mobile/src/api/earn.ts:82-93`）。QuickRecharge tone 明确按 `completed/paid/failed/...` 英文枚举判断（`mobile/src/views/QuickRechargeView.vue:81-85`），Earn action 只识别 `subscribed`（`mobile/src/views/EarnView.vue:293-295`），证明这些不是服务端已本地化 copy。
  - KYC 更严重：mapper 将 approved/rejected 之外的所有新状态直接改成 `pending`（`mobile/src/api/user.ts:301-316`），view label也把任何未知值显示为 pending（`mobile/src/views/KycView.vue:229-233`）。这违反 Mobile localization contract“未知 enum 必须保留，不得替换成错误翻译”（`.trellis/spec/mobile/navigation-and-localization.md:249-256`）。
- **影响**：中文界面出现 `subscribed/completed/...` 等内部英文 token；后端新增 KYC 状态（如 reviewing/expired）会被错误告知“待审核”，可能影响用户下一步操作与客服判断。已知/未知状态的 decision 与 presentation 规则散落，也容易在新增枚举时只更新颜色不更新文案。
- **整改**：为每个领域建立 typed enum parser + presentation adapter：已知值映射双语 key，未知值使用本地化“其他/未知状态”作为主标签并保留 raw value 作为次级诊断；业务 action eligibility 使用 domain enum，不依赖可见 label。KYC adapter不得把未知值改写成 pending。
- **验证**：两种 locale 遍历每个已知状态/category，断言 label/tone/action；注入 `future_status_v2`，断言不显示 pending/成功且 raw source仍可见。AST parity/literal-key scan 纳入 CI。
- **工作量 / 依赖**：M，2–4 天；依赖后端 enum schema/OpenAPI 或明确 allowlist。

### FMD-P2-03 — 根壳与 39 个页面形成重复 main landmark，路由无焦点交接；Seconds listbox 缺键盘模型

- **Delta / 优先级**：新增识别；P2 无障碍与 SPA 路由合同。
- **证据类型**：DOM/handler 缺口是**确认的静态缺陷**；读屏器、浏览器与 Tauri 实际体验未做运行时验证。
- **代码证据**：
  - `App.vue` 用 `<main class="app-stage">` 包住整个 shell 与 RouterView（`mobile/src/App.vue:67-136`）；同时 39 个 view 文件自身存在 `<main>`，例如 Trade（`mobile/src/views/TradeView.vue:1590-1595`）、Wallet Ledger（`mobile/src/views/WalletLedgerView.vue:181`）、KYC（`mobile/src/views/KycView.vue:365`）、Markets（`mobile/src/views/MarketsView.vue:157,221`）。每个常规页面因此至少有外层和页面层两个/嵌套 main landmark。
  - Router 仅定义 lazy routes、scrollBehavior 与 transition beforeEach；没有 route afterEach title/focus/announcement（`mobile/src/router/index.ts:42-99`）。RouterView transition只替换 keyed component，没有可聚焦 route root/live region（`mobile/src/App.vue:118-132`）。通过按钮导航后，旧触发器卸载，浏览器焦点可能落到 body，新页面标题不会被主动读出。
  - Seconds picker声明 `role=listbox` 和 button `role=option`（`mobile/src/views/SecondsView.vue:1316-1329`），但 keydown 只调用通用 modal trap，后者仅处理 Escape/Tab（`mobile/src/views/SecondsView.vue:548-550`、`mobile/src/core/modalDialog.ts:55-80`）；没有 ArrowUp/ArrowDown/Home/End、roving tabindex 或 `aria-activedescendant`。当前源码测试只断言 role 字符串存在（`mobile/tests/seconds-pair-picker-pencil-parity.test.ts:48-66`）。
  - 正面证据：共享 `useModalDialog` 已实现初始焦点、Tab trap、Escape、body lock、focus return 与卸载清理（`mobile/src/core/modalDialog.ts:24-87`），并被多个页面复用；本项不是泛化为“所有 dialog 都失败”。
- **影响**：读屏器 landmark 导航出现歧义；SPA 路由切换缺上下文和焦点起点；Seconds picker宣称 listbox 语义却不提供预期箭头键交互。鼠标/触摸主流程仍可用。
- **整改**：App shell 改为非 landmark container，或只保留一个 route-owned `<main id="main-content">`；每条 route有唯一 `<h1>`。在导航完成后按 policy 聚焦新 route heading/root（`tabindex=-1`），同步 document title，并提供跳到主内容入口。Seconds 要么实现完整 listbox roving/arrow model，要么移除 listbox/option role，使用有 label 的普通按钮列表。
- **验证**：Vue/Playwright + axe 覆盖每页恰好一个可见 main、route click/back/direct-open 后 activeElement 与 title、读屏 live announcement；Seconds 覆盖 ArrowUp/Down/Home/End/Enter/Escape/Tab 和筛选后 active option。重复跑 PWA 与 Tauri WebView。
- **工作量 / 依赖**：M，2–4 天；依赖组件/浏览器 harness 和统一 page heading contract。

### FMD-P2-04 — KYC 预览 Blob URL 从不 revoke，替换/离页会滞留证件图片

- **Delta / 优先级**：新增识别；P2 性能/隐私资源生命周期。
- **证据类型**：**确认的静态缺陷**；具体内存增长依赖图片大小与选择次数。
- **代码证据**：KYC 每次合法文件选择都用 `URL.createObjectURL(file)` 覆盖对应 preview（`mobile/src/views/KycView.vue:281-295::handleFile`），全文件没有 `URL.revokeObjectURL`、`onBeforeUnmount` 或 `onUnmounted` 清理。替换同一 front/back/handheld 时旧 URL 已失去 JS 引用但浏览器 URL registry 仍持有 Blob；离开 route 后当前预览也未显式释放。
- **影响**：反复选择大图可持续占用 WebView/PWA 内存；证件正反面等敏感图像在页面离开后仍由 object URL registry 保持到 document/进程结束。低内存设备可能更早触发页面回收或崩溃。
- **整改**：封装 `replacePreview(kind,file)`，创建新 URL 前 revoke 旧 URL；清除/submit success/route unmount 时 revoke 全部并清空 file input。仅 revoke 本组件创建的 `blob:` URL，避免误处理远端 URL。
- **验证**：mock URL API，选择 front A→front B 断言 A revoke 一次；选择三类后 unmount 断言每个当前 URL 恰好 revoke；非法类型/超限不得 create。浏览器内存 smoke 反复选择大图并确认 Blob 数量不增长。
- **工作量 / 依赖**：S，0.5 天；无后端依赖。

### FMD-P2-05 — 私有 WS 仍只服务 Contract Trade，无入站沉默 watchdog，也不消费 `support.refresh`

- **Delta / 优先级**：延续 MCL-P2-02；P2（提示延迟/恢复体验，REST 保证最终一致性）。
- **证据类型**：owner、事件过滤与 watchdog 缺失是**确认的静态事实**；半开连接频率、hint 实际延迟收益是**运行时假设**。
- **代码证据**：
  - 私有 transport 有 25s ping 和 bounded reconnect（`mobile/src/api/privateUserStream.ts:3-5,239-276`），但 message 只 parse/dispatch，没有 `lastInbound`、pong deadline 或独立 silence timeout；发送 ping 不证明接收链路健康。
  - 唯一实例由 TradeView创建（`mobile/src/views/TradeView.vue:197-202`），event handler 只接受三个 margin position event（`:621-637`），并且仅在 mounted/authenticated/contract mode 时启动（`:640-648`）。parser 虽能保留 `support.refresh`（`mobile/tests/private-user-stream.test.ts:38-43`），生产没有消费者。
  - SupportChat 只用 REST polling controller（`mobile/src/views/SupportChatView.vue:135-174`），固定 5 秒且 single-flight（`mobile/src/core/supportChat.ts:3,158-192`）。backend realtime spec定义 user private channel 的 `support.refresh` 只是 lossy accelerator，仍须 REST 对账（`.trellis/spec/backend/realtime-websockets.md:86-111`）。
  - 正面证据：公开 ticker/detail transport 已使用独立 inbound watchdog（`mobile/src/api/marketTickerStream.ts:91-96,206-258`），并有 open-silent/pong/restore lease 的 fake-socket 测试（`mobile/tests/market-ticker-stream.test.ts:239-278`）；可作为私有连接复用模式。
- **影响**：Contract 页的私有 socket在代理/NAT/radio 半开但仍 `OPEN` 时可长期失去 liquidation accelerator；Support 页永远最多等待下一轮 5 秒 poll，无法利用已存在的提交 hint。Margin/Support 的五秒权威 REST 对账仍能收敛，因此本项不宣称资金状态只存在于 WS，也不升级为 P1。
- **整改**：建立 session-scoped private connection manager + topic lease；Trade 与 Support分别注册 handler，event 只触发序列化 REST reconcile。加入独立 inbound watchdog、jitter、online/visibility 恢复、token epoch 和 connection state；保留五秒轮询作为权威兜底。
- **验证**：OPEN 后静默、pong 丢失、旧 socket迟到、token rotation、最后 lease释放；`support.refresh` 重复 hint 合并为一次 REST，丢全部 hint 后五秒 poll 仍完整恢复。UI 只显示 REST committed state。
- **工作量 / 依赖**：M–L，4–7 天；依赖 FMD-P1-02 session owner 和 fake timer/socket harness。

### 3. 本轮已验证改善 / 未发现新增阻断

1. **功能路由拓扑**：Hash history、route-level lazy import、catch-all、命名回退和安全内部 redirect 当前仍在（`mobile/src/router/index.ts:1-99`）；本轮未找到新的 route name/deep-link/back 业务断裂。无障碍 focus 属于 FMD-P2-03，不等同于功能路由失败。
2. **request cache core**：`createMemoryRequestRegistry` 对成功后 TTL、in-flight dedupe、clone、force、key/global invalidation generation 的实现与行为测试一致（`mobile/src/api/requestCache.ts:20-81`、`mobile/tests/request-cache.test.ts:8-85`）。强一致 wallet/order/ledger/ticker/mutation 未发现新接入该 registry；除 FMD-P1-02 的 session hygiene 外，本轮未单列新的缓存正确性缺陷。
3. **公开 WebSocket liveness**：ticker/detail 已有独立入站沉默 watchdog、当前 socket identity、lease restore 和 fake-timer tests；FMD-P1-01 是 store 启动编排问题，不是否定 transport 本身。
4. **i18n 资源完整性**：zh-CN/en 均 1,744 leaf keys，0 missing、0 placeholder mismatch；1,822 个 literal calls 未发现缺 key。剩余问题是 raw enum/错误 fallback，而非 key 集不对称。
5. **dialog 基础 primitive**：`useModalDialog` 的 initial focus、Tab/Escape、scroll lock 和 focus return 控制流完整；KYC/Seconds 等多数新 picker 已复用。剩余 listbox键盘与 route focus 需另补。
6. **PWA 数据缓存边界**：`runtimeCaching: []`，fallback denylist 排除 API/WS/health/download，Tauri mode 关闭 PWA publicDir，未发现把认证/资金响应写入 Service Worker cache 的新路径。

### 4. Validation record

| 验证 | 结果 |
|---|---|
| `python3 ./.trellis/scripts/task.py current --source` | active task 确认为 `.trellis/tasks/08-30-project-code-business-optimization-reaudit` |
| `npm --prefix mobile run type-check` | PASS，exit 0；注意 tsconfig 排除 tests/src-tauri |
| `npm --prefix mobile test` | PASS：538 total / 538 pass / 0 fail，约 3.5s |
| auth refresh/logout Axios deferred mock | REPRODUCED：logout 后旧请求 replay 200，storage=`ACCESS_NEW`、store为空、split brain=true |
| Wallet Ledger Decimal fixture | REPRODUCED：1e-9/1e-18 显示 0；`9007199254740993.000000000000000001` 映为 9007199254740994 |
| i18n recursive parity | zh-CN 1,744 / en 1,744；0 missing；0 placeholder mismatch |
| literal i18n call scan | 1,822 calls；0 missing literal keys |
| 测试结构盘点 | 90 test files；538 实际 tests；76 files 使用 `readFile/readFileSync`；无 Vue DOM/browser harness dependency |
| 规模盘点 | Trade 6,125；Seconds 3,395；Assets 2,086；共享三 CSS 12,609 行；16 个局部 `@keyframes spin` |
| PWA/Tauri builds | 未运行：研究代理仅允许写 task `research/`，Vite/Tauri build 会写 dist/target；当前构建成功与产物内容保持未知 |

## Files Found

- `.trellis/tasks/08-30-project-code-business-optimization-reaudit/research/mobile-cross-layer.md` — 2026-08-30 Mobile 跨层基线、历史 ID 与当时规模。
- `.trellis/tasks/08-30-project-code-business-optimization-reaudit/prd.md` — 本轮优先级、证据和输出要求。
- `mobile/src/stores/market.ts`、`mobile/src/views/HomeView.vue`、`MarketsView.vue`、`TradeView.vue`、`MarketDetailView.vue` — shared ticker refresh、consumer lease 与路由生命周期。
- `mobile/src/api/client.ts`、`requestAuth.ts`、`auth.ts`、`mobile/src/stores/session.ts`、`mobile/src/App.vue` — token persistence、401 refresh/replay 与 Pinia owner。
- `mobile/src/api/requestCache.ts`、`wallet.ts` — reference TTL registry 与 wallet session-scoped keys。
- `mobile/src/views/OrdersView.vue`、`mobile/src/api/trading.ts`、`mobile/src/core/sessionRequest.ts` — order tab lifecycle、spot cancel-all 与可复用 stale guard。
- `mobile/src/core/walletLedger.ts`、`realizedReturn.ts`、`mobile/src/views/WalletLedgerView.vue` — Ledger Decimal decode、类型与展示。
- `mobile/src/api/seconds.ts`、`swap.ts`、`loan.ts`、`earn.ts`、`prediction.ts`、`newCoin.ts` — 其余 mutation Decimal 边界和 status adapters。
- `mobile/src/views/ReferralsView.vue`、`QuickRechargeView.vue`、`SwapView.vue`、`EarnView.vue`、`KycView.vue` — raw enum、KYC fallback、preview Blob 生命周期。
- `mobile/src/router/index.ts`、`mobile/src/App.vue`、`mobile/src/core/modalDialog.ts`、`mobile/src/views/SecondsView.vue` — route focus、landmarks、dialog 与 listbox keyboard。
- `mobile/src/api/privateUserStream.ts`、`marketTickerStream.ts`、`mobile/src/core/supportChat.ts`、`mobile/src/views/SupportChatView.vue` — private/public WS 与 REST reconciliation。
- `mobile/src/styles/prototype-base.css`、`prototype-parity.css`、`pencil-selected-pages.css`、`mobile/src/components/ContractTradeSheets.vue` — 样式和组件规模热点。
- `mobile/tests/*.test.ts`、`mobile/package.json`、`mobile/tsconfig.json` — Node test 入口、源码合同、测试类型边界和依赖形态。
- `mobile/vite.config.ts`、`mobile/src/pwa/index.ts`、`mobile/src/components/PwaStatus.vue` — PWA shell、更新状态机与 build isolation。
- `mobile/src-tauri/tauri.conf.json`、`mobile/src-tauri/capabilities/default.json` — Tauri build、安全与 capability 配置。
- `scripts/p0-release-gate.sh`、`.github/workflows/docker-image.yml` — Mobile required CI 的直接调用边界。

## Code Patterns

- **错误：busy 即返回，而非 join** — `if (loading.value) return` 让后到调用者误以为数据已就绪；共享资源应持有并返回同一个 in-flight Promise。
- **错误：mutable selection + shared async flags** — await 前后读取/写入同一 `marketTab/stateTab/loading/error`，没有 immutable request key/version，导致旧请求污染当前表面。
- **错误：持久层与响应式层双 owner** — interceptor 写 localStorage、页面读 Pinia，refresh/logout/login 没有 session epoch/CAS。
- **错误：`number -> String(number)` 伪 Decimal 边界** — 原始用户/后端 Decimal 在 string 化前已丢精度。
- **错误：源码存在性当行为证明** — `readFileSync + assert.match` 能锁结构，不能证明 request ordering、DOM focus、service worker 或 cleanup。
- **正确且可复用：generation-aware lifecycle** — `createSessionRequestLifecycle`、wallet ledger lifecycle、margin reconciliation 已展示 stale commit guard。
- **正确且可复用：transport liveness watchdog** — public market ticker/detail 在每个 inbound frame re-arm、旧 timer/socket identity guard、最终 lease 清理。
- **正确且可复用：REST authority + lossy WS hint** — Margin/Support 定时 single-flight REST 对账避免把 WS 当资金/消息事实源。

## External References

- 本轮未访问外部网络；结论依赖当前仓库代码、Trellis specs、lockfile 与本地执行结果。
- `mobile/package-lock.json` 当前解析版本：Vue 3.5.39、Vue Router 4.6.4、Axios 1.18.1、Vite 5.4.21、vite-plugin-pwa 1.3.0、Tauri API 2.11.1、Tauri CLI 2.11.4、TypeScript 5.9.3、vue-tsc 2.2.12、GSAP 3.15.0、lightweight-charts 5.2.0。

## Related Specs

- `.trellis/spec/mobile/backend-integration.md` — auth refresh、market lease、Decimal、request cache、private/public WS、Ledger 和 reconciliation 合同。
- `.trellis/spec/mobile/navigation-and-localization.md` — route/back、locale、未知 enum、dialog/picker 与 browser validation 合同。
- `.trellis/spec/mobile/pwa-and-shell.md` — PWA/Tauri build isolation、shell-only cache、private stream、状态浮岛与产物验证。
- `.trellis/spec/mobile/index.md` — Mobile 必跑 type/test/PWA/Tauri 构建矩阵。
- `.trellis/spec/backend/wallet-amount-precision.md` — 0..18 资产精度与 DECIMAL(38,18) 资金合同。
- `.trellis/spec/backend/spot-orders.md` — server-side cancel-all、部分失败与幂等合同。
- `.trellis/spec/backend/realtime-websockets.md` — `support.refresh` lossy hint 与 REST 权威对账。

## Caveats / Not Found

- 按 Trellis researcher 限制未执行任何 git 命令；“CURRENT HEAD”按当前检出文件快照审计，未把未提交工作区内容与 commit 对象拆分，也不据此声称某缺陷的确切引入提交。
- 未访问生产 API、数据库、真实用户 token、资产 precision 配置、WS 代理、Service Worker installed state 或原生签名渠道。报告明确把触发频率、真实损失和平台行为标为运行时假设。
- 未运行 Vite/Tauri/Android/iOS build，因为它们会在研究目录外写 `dist/target/generated`；FMD-P1-07 证明的是门禁缺失，不是断言当前源码必然构建失败。
- 未做浏览器、axe、读屏器、低端 Android WebView、memory profiler 或 bundle analyzer；FMD-P1-05、P2-03、P2-04 的运行时量化待专门验证。
- 未发现新的功能 route name/deep-link/back 阻断、i18n key 集不对称、强一致 API 被 Service Worker/runtime TTL 缓存、或公开行情 transport 缺入站 watchdog。
- 本轮只新增本研究文件；未修改生产代码、测试、Trellis spec、既有 research、进度日志或其他任务目录。
