# Research: 手机端 SecondsView Header、实时行情与并行订单

- Query: 研究手机端 `SecondsView` 的 Header 重叠根因与当前实时行情/图表刷新链路，比较本项目 `TradeView` / `MarketDetailView` 可复用的本地 WebSocket session/store 模式，并给出并行订单方向的实现与测试建议。
- Scope: internal
- Date: 2026-08-08

## Findings

### 1. Files found

| Path | Description |
| --- | --- |
| `mobile/src/views/SecondsView.vue` | 当前秒合约页面；同时拥有 Header 外置币对选择、REST K 线、行情 store 读取、单个活动订单和下单流程。 |
| `mobile/src/components/PageHeader.vue` | Pencil 二级页 60px sticky Header；中间列当前只渲染文字 copy。 |
| `mobile/src/views/TradeView.vue` | 现货/合约交易页；币对控件直接位于 sticky Header，并使用详情 WebSocket session。 |
| `mobile/src/views/MarketDetailView.vue` | 行情详情页；展示完整的 REST + WebSocket 并发启动、竞态合并、价格优先级和 session 生命周期。 |
| `mobile/src/stores/market.ts` | 多币种 ticker Pinia store；REST 快照后可启动共享 ticker WebSocket。 |
| `mobile/src/api/marketSocket.ts` | 模块级单连接、多 symbol ticker 订阅，含心跳与指数退避重连。 |
| `mobile/src/api/marketDetailStream.ts` | 单 symbol 的 depth/trade/kline stream 与带 generation/request token 的 session 封装。 |
| `mobile/src/api/marketSocketProtocol.ts` | 行情帧解析、symbol/interval 归一化及 K 线合并逻辑。 |
| `mobile/src/api/market.ts` | ticker、K 线、盘口、成交 REST 快照入口。 |
| `mobile/src/config/app.ts` | 手机端当前仅暴露 `/ws/public` 的 WebSocket URL helper。 |
| `mobile/src/api/seconds.ts` | 秒合约产品、订单列表和开仓适配器。 |
| `mobile/src/core/secondsOrder.ts` | 秒合约订单字段和时间戳映射。 |
| `src/modules/seconds_contract/application.rs` | 后端开仓事务；没有“已有活动单即拒绝”的业务限制。 |
| `src/modules/seconds_contract/service.rs` | 后端发布 `seconds_contract.order.opened/settled` 私有事件。 |
| `src/workers/seconds_contract_settlement.rs` | 自动结算后发布私有 settled 事件。 |
| `src/modules/events/routes.rs` | 已存在 `/ws/seconds` 和 `/ws/private?token=...` 路由。 |
| `pc/src/views/SecondOptions.vue` | PC 秒合约既有模式：seconds ticker 订阅、私有事件触发订单/余额刷新、逐订单倒计时。 |
| `pc/src/stores/second.ts` | PC 秒合约 store；持有 ticker、`currentOrders[]`、历史和余额。 |
| `mobile/tests/market-detail-stream.test.ts` | 可执行 fake-socket/session 竞态、重连、停止和 ABA 隔离测试。 |
| `mobile/tests/market-socket.test.ts` | 行情订阅帧、严格解析和 K 线去重/合并测试。 |
| `mobile/tests/pencil-trading-product-selected-parity.test.ts` | 当前锁定单个 `activeOrder`、REST sparkline 与秒合约几何的源码合同。 |
| `mobile/tests/priority-secondary-page-parity.test.ts` | 当前锁定 PageHeader、`seconds-pair-field` 顺序和秒合约流程的源码合同。 |
| `mobile/tests/award-ui-trading-workspaces.test.ts` | 当前交易工作台结构、尺寸、真实 API 与响应式合同。 |
| `.trellis/spec/mobile/backend-integration.md` | 手机端 WebSocket、REST/WS 竞态、Seconds mutation commit point 的执行合同。 |
| `.trellis/spec/mobile/index.md` | sticky Header、手机宽度、图表与共享控件合同。 |
| `.trellis/spec/backend/realtime-websockets.md` | `/ws/public`、`/ws/seconds`、`/ws/private` 的后端路由和业务隔离合同。 |
| `.trellis/spec/backend/seconds-contracts.md` | 秒合约产品、周期、订单时间戳、钱包和结算合同。 |

### 2. Header 重叠根因

#### 2.1 当前 DOM 同时渲染了两个中间标题所有者

`SecondsView` 把 `selected.symbol` 传给 `PageHeader` 的标题（`mobile/src/views/SecondsView.vue:445-459`），紧接着又渲染一个包含同一币对的 `seconds-pair-field`（`mobile/src/views/SecondsView.vue:461-477`）。因此同一屏幕中同时存在：

1. `PageHeader` 中间列的 `.page-header__title`；
2. Header 外部的原生 `<select>`。

`PageHeader` 的 Pencil 模式本身是 `44px / minmax(0, 1fr) / 44px` 三列、60px 高、`position: sticky; top: 0; z-index: var(--layer-sticky-header)`（`mobile/src/components/PageHeader.vue:80-95`），标题明确占中间列（`mobile/src/components/PageHeader.vue:115-140`）。

#### 2.2 外置 select 被绝对定位回同一 60px Header 区域

`.seconds-page` 是定位容器（`mobile/src/views/SecondsView.vue:773-779`）。`.seconds-pair-field` 使用：

```css
left: 72px;
right: 72px;
top: 4px;
position: absolute;
z-index: calc(var(--layer-sticky-header) + 1);
```

见 `mobile/src/views/SecondsView.vue:799-807`。其内部 shell 和 `<select>` 都是 52px 高（`mobile/src/views/SecondsView.vue:809-843`），所以垂直范围正好是 `4px..56px`，完全落入 60px Header 内；z-index 又比 Header 高一层。重叠不是随机 WebView 绘制问题，而是确定的布局结果：**外置币对选择器覆盖 Header 中间标题，两个元素仍同时存在并参与可访问树/焦点行为。**

#### 2.3 可复用的正确本地模式

- `TradeView` 将币对按钮直接放在 `.spot-pencil-header` 的中间网格列中（`mobile/src/views/TradeView.vue:607-641`），Header 自己是唯一 sticky/stacking owner（`mobile/src/views/TradeView.vue:1217-1229`）。
- `MarketDetailView` 同样把 `.market-detail__instrument` 放入 Header 网格中（`mobile/src/views/MarketDetailView.vue:476-505`），Header 与 instrument 的几何由同一 grid 控制（`mobile/src/views/MarketDetailView.vue:792-847`）。

结论：Seconds 不应继续用“通用 PageHeader + Header 外绝对定位选择器”的双所有者结构。

### 3. SecondsView 当前行情与图表数据流

#### 3.1 首屏 REST 链路

```text
onMounted
  ├─ initializeSparkline()
  ├─ 启动 1 秒 UI clock
  └─ Promise.all([
       load(),                 // products + orders + wallet
       marketStore.refresh()  // GET markets ticker snapshot
     ])

load()
  ├─ GET /seconds-contracts/products
  ├─ 登录态并行 GET /seconds-contracts/orders + GET /wallet/accounts
  ├─ 取 orders 中第一个 opened/pending/active 订单
  ├─ 优先把 selected 切到该订单 symbol
  └─ fetchKlines(selected.symbol, '1m')，仅保留最后 48 点
```

对应代码：

- `selectedTicker` 只是 `marketStore.tickerFor(selected.symbol)`（`mobile/src/views/SecondsView.vue:66-74`）。
- sparkline 是一次性 `fetchKlines(symbol, '1m')`，仅用 `chartRequestVersion` 防止切币后的旧 REST 覆盖（`mobile/src/views/SecondsView.vue:136-147`）。
- products/orders/accounts 的聚合与“活动单驱动 selected”逻辑位于 `mobile/src/views/SecondsView.vue:218-247`。
- mount 只调用 `marketStore.refresh()`，没有 `startLiveUpdates()`，见 `mobile/src/views/SecondsView.vue:416-428`。

#### 3.2 当前并非实时行情/实时图表

`marketStore.refresh()` 是 REST 快照，并在 20 秒内拒绝非 force 重拉（`mobile/src/stores/market.ts:16-29`）。真正的 ticker WebSocket 入口是 `marketStore.startLiveUpdates()`（`mobile/src/stores/market.ts:32-54`），但 SecondsView 没有调用它。Home/Markets 离开时还会显式 `stopLiveUpdates()`，因此不能依赖前一个页面残留的连接（`mobile/src/views/HomeView.vue:189-198`、`mobile/src/views/MarketsView.vue:139-151`）。

所以当前页面表现为：

- 大价格：进入页面时的 market REST 快照，之后不会因 SecondsView 自身而更新；
- 微型图：进入/切币/订单列表重载时的一次 REST K 线快照，不接收 forming candle；
- 1 秒 timer：只更新倒计时，并在一个活动单到期时触发 `load()`，不是行情刷新器（`mobile/src/views/SecondsView.vue:419-426`）。

#### 3.3 当前提交后的刷新边界还有既有合同风险

开仓成功后，页面先按 id upsert 返回订单，但随后在同一个 `try` 中 `await load()`（`mobile/src/views/SecondsView.vue:303-330`）。如果订单已经创建、后续列表或钱包刷新失败，外层 `catch` 会把整个操作显示为下单失败。这与 `.trellis/spec/mobile/backend-integration.md:247-251` 的 mutation commit point 合同冲突；成功返回订单必须保留，后续 reconciliation 失败只能是刷新告警。

### 4. 并行订单现状与瓶颈

#### 4.1 后端数据模型已经允许多个 opened 订单

- 开仓流程只校验幂等键、产品/周期、ticker、余额后插入订单，没有查询或拒绝同用户既有活动单（`src/modules/seconds_contract/application.rs:235-373`）。
- 用户订单列表按新到旧返回多个订单；后端测试直接插入并断言同一用户两个 `opened` 订单均返回（`tests/seconds_contract_routes.rs:1744-1852`）。

#### 4.2 手机端把数组压成单个全局活动单

- `activeOrder` 使用 `.find(...)`，只取第一个活动状态（`mobile/src/views/SecondsView.vue:72-83`）。
- 任意活动单存在时，方向、周期、金额和提交按钮全部用 `Boolean(activeOrder)` 禁用（`mobile/src/views/SecondsView.vue:559-620`、`mobile/src/views/SecondsView.vue:627-673`）。
- 活动态只渲染一个 card（`mobile/src/views/SecondsView.vue:516-549`）。
- `load()` 会把当前选择强制切到第一个活动单的产品（`mobile/src/views/SecondsView.vue:230-239`），多个 symbol 时会夺走用户当前选择。
- 到期刷新只监视这一个 `activeOrder`，并用单个 `expiredReloadedOrderId` 去重（`mobile/src/views/SecondsView.vue:63-64,419-425`）。

因此“只能有一个活动单”是手机 UI 状态建模和 disabled 条件造成的，不是后端约束。

### 5. 可复用 WebSocket/store 模式比较

| 模式 | 本地实现 | 优点 | 局限 | Seconds 适配度 |
| --- | --- | --- | --- | --- |
| 多 symbol ticker store | `mobile/src/stores/market.ts` + `mobile/src/api/marketSocket.ts` | Pinia 响应式；REST 决定列表结构；一个 socket 订阅多个 symbol；心跳和有界退避；适合 Home/Markets。 | 只更新 latest price/change；不更新 K 线；启动时订阅整份 markets 列表；`stopLive` 是单所有者开关，不是引用计数 lease；无法解决图表 REST/WS 竞态。 | 只修大价格时最小，但不能单独完成“价格 + 图表实时”。 |
| 单 symbol detail session | `mobile/src/api/marketDetailStream.ts` | 同时订阅 depth/trade/kline；`replace()` 先停旧连接；symbol + interval + requestVersion + generation 隔离；K 线 live 优先合并；RAF 合并高频帧；完整 cleanup/reconnect。 | 当前固定订阅三类数据，Seconds 不需要 depth；每个页面拥有独立 socket；默认 URL helper 是 `/ws/public`。 | 最适合 Seconds 的选中 symbol + 1m sparkline，可同时给出 live trade price 和 forming candle。 |
| TradeView 页面用法 | `mobile/src/views/TradeView.vue:168-267,551-595` | 页面状态直接消费 session；REST 与 socket 同时启动；切 symbol/interval 时 replace；unmount stop。 | `loadMarketData()` 会清空盘口/成交，逻辑偏完整交易台。 | 复用 session 和生命周期，不复制整页状态。 |
| MarketDetailView 页面用法 | `mobile/src/views/MarketDetailView.vue:90-267,457-467` | 价格优先级明确为 live trade -> live/forming candle -> ticker snapshot；晚到 REST 不覆盖 live；interval 局部替换。 | 完整详情页还包含盘口、成交、指标和全屏图表。 | 是 Seconds 实时价格/微图的首选参考。 |
| PC Seconds 专用模式 | `pc/src/views/SecondOptions.vue:70-123,218-274,310-345` + `pc/src/stores/second.ts:56-185` | `/ws/seconds` 行情隔离；`currentOrders[]`；私有 opened/settled/wallet 事件触发订单与余额 reconciliation；逐订单倒计时。 | PC client/service 不能直接复制到 mobile；当前事件处理收到一条就全量刷新，仍可进一步合并。 | 可参考订单数组和私有事件方向，不应复制 PC 的宽松 `any` 映射。 |

`marketDetailStream` 的关键可执行保证已由 fake socket 测试覆盖：三 channel 订阅、symbol 过滤、重连和幂等停止（`mobile/tests/market-detail-stream.test.ts:113-210`），latest-only K 线 RAF（`mobile/tests/market-detail-stream.test.ts:331-408`），以及 live/REST、interval、request、generation、A-B-A 隔离（`mobile/tests/market-detail-stream.test.ts:443-612`）。

### 6. Recommended implementation

#### 6.1 Header：一个 sticky owner，一个中间交互控件

推荐给 `PageHeader` 增加保持默认 copy 行为的可选 center/title slot，Seconds 把币对选择控件放入 Header 中间网格列；未传 slot 的所有现有页面继续渲染当前 title/eyebrow/subtitle。随后删除 Header 外的 `.seconds-pair-field` 绝对定位规则。

选择该方案的理由：

- 保留共享返回和 action 逻辑；
- 结构与 TradeView/MarketDetailView 的“instrument 是 Header 子项”一致；
- 不需要 z-index 补丁或人为隐藏被覆盖的标题；
- 选择器、返回按钮、订单按钮由同一个 60px grid 决定几何和焦点顺序。

如果不扩展共享组件，则第二选择是 Seconds 专用 Header，直接仿照 `TradeView` 的三列结构。不要保留“PageHeader + 绝对定位 select”并仅调整 z-index/margin；那只会改变谁覆盖谁，双标题和可访问树问题仍在。

#### 6.2 行情/微图：复用 detail session，不再额外开全市场 ticker socket

推荐在 SecondsView 中复用 `createMarketDetailStreamSession`：

1. 固定 interval 为 `1m`，选中产品 symbol 变化时递增页面 request version 并调用 `replace(symbol, '1m', version)`。
2. 在同一时刻启动 `fetchKlines(symbol, '1m')`，通过 `beginKlineRequest/resolveKlineRequest` 合并；live candle 对同一 `open_time` 保持权威。
3. `onKlines` 更新 `sparklinePoints`；`onTrade` 更新最近 live trade price；`onDepth` 可为空操作。若后续认为无用 depth 流量不可接受，再把底层 session 抽象为可配置 channels，而不是在 SecondsView 新写一套 socket/reconnect。
4. 页面参考价采用 `latest live trade -> latest Kline close -> marketStore ticker REST snapshot -> --`，与 `MarketDetailView` 的既有优先级一致（`mobile/src/views/MarketDetailView.vue:100-106`）。
5. `marketStore.refresh()` 仍可作为 ticker 元数据/24h 快照 fallback，但 Seconds 不再调用 `marketStore.startLiveUpdates()`，避免同时维护“全市场 ticker socket + 单 symbol K 线 socket”。
6. 切币先停旧 generation；unmount 递增版本并 `session.stop()`，同时取消 ResizeObserver/MutationObserver/timer。

URL 建议分两层：

- 最小改动：继续注入 `publicMarketWebSocketUrl`，完全符合当前 mobile backend-integration 合同。
- 若本任务同时要求业务连接隔离：新增由同一 runtime config 派生的 `/ws/seconds` URL helper，再把它注入相同 session。后端路由已经存在（`.trellis/spec/backend/realtime-websockets.md:11-22`），但这需要同步 mobile URL 测试和 spec；不要在 view 中手拼 URL。

#### 6.3 并行订单：保持数组语义，市场 stream 与订单 stream 分责

1. 用 `activeOrders = orders.filter(isActiveStatus)`，不要再把状态压成一个 `activeOrder`。
2. 当前选中产品由用户选择保持；初次无选择时才默认第一个产品。活动订单不得在每次 reconciliation 后强制改变 Header symbol。
3. prominent active 区域可以渲染当前 symbol 的多个活动单；“我的订单”继续展示全 symbol。每个订单独立计算 remaining/progress/estimatedProfit。
4. 表单 disabled 只依赖 `loading/submitting/selected/cycle/valid`，不依赖“是否存在任意活动单”。后端余额仍是最终并发保护。
5. 1 秒 clock 只更新本地倒计时。对已到期活动单用 `Set<orderId>` 记录 in-flight/reconciled id，并把同一 tick 的多个到期订单合并成一次 `fetchSecondsOrders()` + `fetchWalletAccounts()`，避免 N 个订单触发 N 次全量刷新。
6. `openSecondsOrder()` 返回即按 id upsert、关闭确认、显示成功并释放本次 submitting；订单/钱包 reconciliation 独立 best-effort，失败显示 refresh warning，不能撤销成功订单或改成“下单失败”。
7. 私有订单实时性不要塞进 market detail session。若需要 worker 结算即时到达，新增独立、token-scoped 的 mobile private-event session，过滤 `seconds_contract.order.opened/settled` 和相关 `wallet.*` 后触发合并 reconciliation。后端事件已存在（`src/modules/seconds_contract/service.rs:80-159`、`src/workers/seconds_contract_settlement.rs:407-429`），但 mobile 当前没有私有 WebSocket client；因此也可先用“到期批量轮询 + 页面进入刷新”作为本任务的最小闭环。

### 7. Test points

#### Header / layout

- 结构测试：Seconds 的币对选择器必须是 Header 中间列后代；页面内只出现一个可见币对标题；删除 `.seconds-pair-field { position: absolute; top: 4px; z-index: ... }` 合同。
- runtime：320x720、360x800、390x844、448x900，light/dark；初始、滚动、聚焦 select、打开订单区均无 Header 文本重叠、横向溢出或 action 点击遮挡。
- sticky：滚动时 Header 保持 60px 和 `z-index: 70`，内容不得进入 Header stacking layer；返回/订单按钮仍为 44x44。
- 可访问性：焦点顺序为返回 -> 币对选择 -> 我的订单；选择器有唯一 label，不能保留被覆盖的重复 title。

#### Market WebSocket / chart

- 基于 `market-detail-stream.test.ts` 的 fake socket 测试 Seconds 1m session：发送 trade/Kline 后价格和 sparkline 更新，Resize 后 canvas 仍绘制最新 points。
- REST/WS race：live forming candle 先到、REST 后到时，同 `open_time` 的 live close 获胜。
- symbol A -> B -> A：旧 socket frame、旧 REST promise、已取消 RAF 均不能写入当前图。
- WebSocket 临时失败：REST sparkline 和 ticker fallback 保留；指数退避后重订阅；stop 后不重连。
- 严格坏帧：错误 symbol/interval、非法 OHLCV、空 provider 不清空最后有效图。
- 价格优先级：trade > Kline close > ticker snapshot > unavailable。
- 若使用 `/ws/seconds`：PWA dev proxy、Tauri/product origin、ws/wss scheme 和 nested `/api/v1/ws/seconds` 路径均需测试。

#### Multiple orders / mutation

- 两个同时 `opened` 订单渲染两个独立倒计时/进度；其中一个到期不移除或冻结另一个。
- 已有活动单时方向、周期、金额和提交按钮仍可用；连续两次成功提交得到两个不同 id，列表按 id upsert 且不重复。
- 多 symbol 活动单加载后不强制改写用户当前 selected product。
- 同一 tick 多个订单到期只触发一次订单/钱包 reconciliation；刷新完成后 settled 订单移动到历史状态。
- create 成功、随后 orders 或 wallet 刷新失败：新订单和成功反馈保留，只显示 refresh warning，且不会重复调用 create。
- 快速连点/确认关闭：每次 confirmation 只允许一个 in-flight mutation；idempotency key 仍逐次唯一。
- 私有事件方案若纳入：只接受当前 token 用户，过滤无关事件，断线重连，logout/token 变化立即关闭旧连接，opened/settled 事件风暴要合并刷新。

#### Existing tests that must be revised

- `mobile/tests/pencil-trading-product-selected-parity.test.ts:69-92` 当前强制单个 `activeOrder` 和一次性 `fetchKlines`。
- `mobile/tests/priority-secondary-page-parity.test.ts:121-164` 当前强制 `seconds-pair-field` 位于 `seconds-content` 前。
- `mobile/tests/award-ui-trading-workspaces.test.ts:46-141` 当前锁定 pair field、尺寸和 store ticker 源码形状。
- `mobile/tests/android-ui-trading-prototype-v16.test.ts:65-84` 当前只验证单个 active order 和静态 ticker getter。

实现后最接近的完整验证命令：

```bash
npm --prefix mobile run type-check
npm --prefix mobile test
npm --prefix mobile run build:pwa
npm --prefix mobile run build:tauri
git diff --check
```

### 8. External references / versions

- 未使用外部网络资料；结论完全来自本地代码、测试和 Trellis specs。
- 本地 `mobile/package.json` 固定 `klinecharts@10.0.0`、`lightweight-charts@5.2.0`；本任务的微型 canvas 不需要直接引入任一 renderer，session 输出仍应保持统一的 `KlinePoint[]`。
- 本地声明 `pinia ^2.1.0`、`vue ^3.4.0`；lockfile 当前解析 Vue `3.5.39`。

### 9. Related specs

- `.trellis/spec/mobile/index.md`: Shared Header、sticky layer、手机宽度、实时图表和 interval session 合同。
- `.trellis/spec/mobile/backend-integration.md:164-235`: mobile market WebSocket、session identity、REST/WS 合并和价格优先级。
- `.trellis/spec/mobile/backend-integration.md:237-251,336-394`: Seconds adapter、mutation commit point、错误矩阵和测试要求。
- `.trellis/spec/backend/realtime-websockets.md`: business-scoped `/ws/seconds` 与 token-scoped `/ws/private`。
- `.trellis/spec/backend/seconds-contracts.md`: 多周期产品、订单时间戳、共享现货钱包、entry/settlement price。
- `.trellis/spec/guides/code-reuse-thinking-guide.md`: 优先扩展现有 session/Header，而不是复制 socket/reconnect。
- `.trellis/spec/guides/cross-layer-thinking-guide.md`: UI -> mobile adapter -> REST/WS -> backend event/settlement 的边界检查。

## Caveats / Not Found

- `task.py current --source` 返回 `Current task: (none)`；本文件位置使用用户明确提供的 task path，未修改 task 状态。
- 任务目录当前只有 `task.json`、`implement.jsonl`、`check.jsonl`，没有 `prd.md`；“并行订单”的具体展示上限、是否允许跨 symbol 同时下单、是否本期必须接私有 WebSocket 尚无 PRD 决策。
- mobile 当前没有 `/ws/private` client/store，也没有 Seconds 专用 `/ws/seconds` URL helper；两者都不能仅靠改 `SecondsView` 获得，需要明确纳入实现范围和补充 spec/test context。
- `createMarketDetailStreamSession` 当前总会订阅 depth/trade/kline。直接复用会接收 Seconds 未展示的 depth；若要最小网络负载，需要在共享 API 层增加可配置 channel，而不是在 view 中复制 socket 实现。
- 本次为静态代码研究，未启动 PWA/Tauri、未连接真实 WebSocket、未做真机滚动截图；Header 根因由确定的 DOM/CSS 几何推导，运行时验证仍应列入实现阶段。
