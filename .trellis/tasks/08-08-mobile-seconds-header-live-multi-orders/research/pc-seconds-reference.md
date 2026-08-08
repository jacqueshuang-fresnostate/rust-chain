# Research: PC 秒合约 Header、实时行情与多活动订单参考

- Query: 研究 PC 端秒合约的 Header、实时 ticker/K 线更新及多活动订单展示模式，找出可复用且不依赖外部 TradingView 的本地实现，并与当前 Mobile 秒合约实现比较。
- Scope: internal
- Date: 2026-08-08

## Findings

### 结论摘要

PC 秒合约值得复用的是数据与状态组织方式，而不是整页组件：页面以秒合约产品决定可交易交易对，先用 REST ticker 建立快照，再对每个秒合约交易对订阅 `seconds` ticker；K 线通过统一的内部 REST + WebSocket 数据管线更新；活动订单以数组逐行展示，并以订单 ID 独立维护倒计时和结算查询锁，因此前端没有“已有一单就禁止继续下单”的限制。

对 Mobile 的最佳本地方案不是移植 PC 的 `@klinecharts/pro` 组件，而是复用 Mobile 已有的本地 `klinecharts@10.0.0` 渲染器、行情协议/合并函数和详情流会话。现有 Mobile 图表渲染层只消费传入的本地 `KlinePoint[]`，不自行请求网络，不创建 iframe、远程 script 或外链，已经满足“不依赖外部 TradingView”。秒合约页面只需接入 ticker/K 线数据所有权，并把单一 `activeOrder` 模型改为 `activeOrders`。

### Files found

| Path | Description |
| --- | --- |
| `pc/src/views/SecondOptions.vue` | PC 秒合约整页：页内 ticker Header、秒合约交易对列表、K 线、活动/历史订单表、下单与逐单倒计时。 |
| `pc/src/stores/second.ts` | PC 秒合约 Pinia store；持有 ticker、周期、当前订单、历史订单和余额，并兼容多种 ticker 字段。 |
| `pc/src/api/second.ts` | PC 秒合约 REST 适配；产品决定交易对，再按交易对补 ticker；订单按状态/交易对过滤。 |
| `pc/src/api/backendAdapters.ts` | 秒合约产品、ticker、周期、订单和下单请求的 PC DTO 映射。 |
| `pc/src/api/stomp.ts` | PC 公共/私有 WebSocket 管理；spot、margin、seconds 独立连接、订阅复用、断线重订阅和消息适配。 |
| `pc/src/components/chart/MarketChart.vue` | PC 图表包装器；按配置懒加载 KLineCharts 或 lightweight-charts 渲染器。 |
| `pc/src/components/chart/TVChart.vue` | PC 的本地 KLineCharts Pro 渲染器；内部 REST 历史 + `seconds:kline` WebSocket。文件名虽为 TVChart，实际不是 TradingView。 |
| `pc/src/components/chart/klineData.ts` | PC K 线模块/周期/topic、历史/实时 bar 和时间戳标准化。 |
| `pc/src/components/chart/klineDataSource.ts` | 按 spot/margin/seconds 选择内部 K 线 REST fetcher。 |
| `pc/tests/stomp.test.ts` | seconds 独立 WS、ticker/K 线订阅、消息路由和重连隔离的可执行参考。 |
| `pc/tests/kline-data.test.ts` | K 线 topic、周期、秒/毫秒时间戳、去重和排序合同。 |
| `pc/tests/backendAdapters.test.ts` | 秒合约产品去重、禁用产品过滤、ticker 丰富、周期和订单映射合同。 |
| `mobile/src/views/SecondsView.vue` | 当前 Mobile 秒合约；REST ticker/微型静态折线、单一活动订单、最多三条订单摘要。 |
| `mobile/src/api/marketSocket.ts` | Mobile 已有单连接多 ticker 订阅、心跳、指数退避重连和清理。 |
| `mobile/src/api/marketSocketProtocol.ts` | Mobile 已有 ticker/K 线订阅帧、严格帧解析、K 线去重/排序/合并。 |
| `mobile/src/api/marketDetailStream.ts` | Mobile 已有实时 K 线会话、REST/WS 竞态隔离、逐帧合并、重连和生命周期清理。 |
| `mobile/src/components/MobileMarketChart.vue` | Mobile 本地图表包装器；渲染已归一化的 `KlinePoint[]`。 |
| `mobile/src/components/KLineChartMarketChart.vue` | 推荐复用的本地 `klinecharts@10.0.0` 渲染器；内存 DataLoader，无远程数据源。 |
| `mobile/tests/market-detail-reference-layout.test.ts` | 锁定本地图表版本、内存 loader、无外链/远程脚本和渲染生命周期。 |

### PC Header 模式

1. PC 全局导航 Header 与秒合约行情 Header 是两层结构。全局 Header 负责品牌、产品入口、账户与语言；秒合约页内 Header 才负责当前交易对行情。全局 Header 的秒合约入口位于 `pc/src/components/layout/Header.vue:57-65`，页内行情 Header 位于 `pc/src/views/SecondOptions.vue:353-386`。
2. 页内行情 Header 同时展示交易对 logo/符号、秒合约 badge、最新价、折算价、24h 涨跌、24h 高点和低点；正负色由 ticker 的 `chg` 决定（`pc/src/views/SecondOptions.vue:357-384`）。其数据源是 `currentTicker = store.getTickerBySymbol(symbol)` 与 `currentPrice = currentTicker.close`（`pc/src/views/SecondOptions.vue:36-45`）。
3. 秒合约可交易交易对不是全市场列表。PC 先读取 `/seconds-contracts/products`，过滤 active 产品、按规范化 symbol 去重，然后逐 symbol 请求内部市场 ticker 作为首屏快照（`pc/src/api/second.ts:37-45`、`pc/src/api/second.ts:108-128`；去重映射见 `pc/src/api/backendAdapters.ts:1030-1072`）。这符合秒合约 spec 对产品源的要求，而不会把不可做秒合约的现货对混入选择器。
4. 当前 Mobile 的 `PageHeader` 只显示交易对标题和历史按钮（`mobile/src/views/SecondsView.vue:445-459`）；Pencil Header 会隐藏 eyebrow/subtitle 并固定为 60px 三轨结构（`mobile/src/components/PageHeader.vue:80-95`、`mobile/src/components/PageHeader.vue:126-139`）。行情板只显示最新价、周期和赔率（`mobile/src/views/SecondsView.vue:491-513`），没有 24h 涨跌/高/低。

**可复用判断：**复用 PC 的“pair identity + latest + 24h stats”信息层级和 seconds-products-first 数据源；不要直接复制 PC 的桌面横向 Tailwind Header。Mobile 应保留 `PageHeader` 的 60px sticky/返回/44px action 合同，把行情统计放在其下方的紧凑 ticker strip 或现有 market board 顶部，避免破坏共享 Header 几何。

### 实时 ticker 模式

1. PC 加载秒合约产品和周期后，对产品列表中的每个唯一 symbol 建立 `seconds:ticker:<symbol>` 订阅；回调解析 payload 并写回秒合约 store（`pc/src/views/SecondOptions.vue:107-124`、`pc/src/views/SecondOptions.vue:313-323`）。切换 symbol 只改变当前观察对象和订单过滤，不需要重建全部 ticker 订阅（`pc/src/views/SecondOptions.vue:95-105`）。
2. `StompService` 把 `seconds` 映射到独立 `/ws/seconds` 客户端（`pc/src/api/stomp.ts:642-650`）；订阅以 `channel + normalized symbol + interval` 去重，多 callback 共用一条服务端订阅（`pc/src/api/stomp.ts:149-173`、`pc/src/api/stomp.ts:264-290`）。断线时只重连仍有订阅的业务客户端，并重新发送订阅，不影响 spot/margin（`pc/src/api/stomp.ts:103-121`、`pc/src/api/stomp.ts:426-432`）。隔离行为由 `pc/tests/stomp.test.ts:255-358` 覆盖。
3. ticker payload 经过 symbol 规范化，并兼容 `close/last/price/last_price`、`open/open_24h`、`high/high_24h`、`low/low_24h`、`volume/volume_24h` 与 `price_change_percent_24h` 等字段；更新时保留现有 icon/显示 symbol（`pc/src/stores/second.ts:91-150`）。
4. 当前 Mobile `SecondsView` 只在 mount 时并行执行 `load()` 和 `marketStore.refresh()`（`mobile/src/views/SecondsView.vue:416-428`），没有调用 `marketStore.startLiveUpdates()`，因此该页的 `selectedTicker` 只是 REST 快照。Mobile 已有 ticker 连接实现：单连接多 symbol、25 秒心跳、1–30 秒指数退避、无人监听时关闭（`mobile/src/api/marketSocket.ts:13-23`、`mobile/src/api/marketSocket.ts:41-63`、`mobile/src/api/marketSocket.ts:87-108`）。

**可复用判断：**Mobile 不需要移植 PC `StompService`。直接复用现有 `subscribeTickers()` 和 `parseMarketSocketFrame()` 即可；订阅 symbol 集合应来自秒合约产品。若通过 `marketStore.startLiveUpdates()` 接入，它会订阅全市场且是单一 stop handle（`mobile/src/stores/market.ts:32-54`），不如秒合约页面直接持有面向产品 symbol 的订阅清理函数精确。Mobile 继续使用 `/api/v1/ws/public`，不要照搬 PC 根路径 `/ws/seconds`，以遵守现有 PWA/Tauri URL 与代理合同。

### 实时 K 线与本地图表模式

1. PC 秒合约把 `MarketChart` 配置为 `module="seconds"`、`period="1m"`（`pc/src/views/SecondOptions.vue:440-450`）。`MarketChart` 只做渲染器懒加载和统一 props 传递（`pc/src/components/chart/MarketChart.vue:1-12`、`pc/src/components/chart/MarketChart.vue:35-44`）。
2. PC 的 seconds 历史 K 线复用内部市场 K 线 REST，不使用外部行情：`fetchSecondKlineHistory = fetchHistoryKLine`（`pc/src/api/second.ts:15-19`），模块选择器将 seconds 路由到该 fetcher（`pc/src/components/chart/klineDataSource.ts:6-17`）。实时 topic 默认生成 `seconds:kline:<symbol>:<interval>`（`pc/src/components/chart/klineData.ts:15-37`）。
3. PC `TVChart.vue` 实际使用本地 `@klinecharts/pro`，其 datafeed 的 history callback 调内部 REST，subscribe callback 调内部 seconds WebSocket；实时 bar 直接推给图表 callback（`pc/src/components/chart/TVChart.vue:45-115`）。历史与实时数据共享同一标准化层，负责秒/毫秒时间戳、无效值过滤、去重和排序（`pc/src/components/chart/klineData.ts:51-82`、`pc/src/components/chart/klineData.ts:98-124`；测试见 `pc/tests/kline-data.test.ts:13-56`）。
4. PC 的 `TradingViewChart.vue` 虽然数据仍来自内部 REST/WS，但模板显式包含 `https://www.tradingview.com/` attribution 外链（`pc/src/components/chart/TradingViewChart.vue:1-9`），不适合作为“无外部 TradingView”目标。
5. Mobile 已有更合适的纯本地实现：`MobileMarketChart` 只把归一化 points 传给当前引擎（`mobile/src/components/MobileMarketChart.vue:34-39`、`mobile/src/components/MobileMarketChart.vue:158-174`）；默认 KLineCharts 渲染器使用 `klinecharts@10.0.0` 的内存 `DataLoader`，数据只来自父组件的 `points`（`mobile/src/components/KLineChartMarketChart.vue:181-195`、`mobile/src/components/KLineChartMarketChart.vue:298-326`），并在卸载时 disconnect observer 和 dispose chart（`mobile/src/components/KLineChartMarketChart.vue:410-420`）。
6. Mobile 的行情协议层已经实现 K 线订阅帧、严格实时 payload 校验、timestamp 归一化、去重排序和 160 点保留（`mobile/src/api/marketSocketProtocol.ts:41-73`、`mobile/src/api/marketSocketProtocol.ts:75-125`、`mobile/src/api/marketSocketProtocol.ts:239-257`）。详情流实现了 heartbeat、指数退避、重订阅、逐 animation frame 合并和 stop 清理（`mobile/src/api/marketDetailStream.ts:129-181`、`mobile/src/api/marketDetailStream.ts:183-323`）；会话层保证 symbol/interval/request generation 隔离，并让实时 candle 覆盖迟到的 REST candle（`mobile/src/api/marketDetailStream.ts:326-415`、`mobile/src/api/marketDetailStream.ts:417-449`）。

**可复用判断：**Seconds 页面应复用 Mobile 的 `MobileMarketChart`/`KLineChartMarketChart`、`fetchKlines()`、`marketSocketProtocol` 和 `mergeMarketKlines()`。`startMarketDetailStream()` 当前固定订阅 depth、trade、kline（`mobile/src/api/marketDetailStream.ts:243-249`），若秒合约只需要 ticker + K 线，不宜原样调用并产生无用 depth/trade 订阅；更合适的是复用其会话/竞态/清理模式，或抽出可选 channels。图表数据仍应由页面/stream owner 持有，渲染器保持无网络职责。

### 多活动订单模式

1. PC 的活动订单是完整数组 `currentOrders`，不是单一 active order（`pc/src/stores/second.ts:56-64`）。当前 symbol 的 open/opened/pending 订单全部由 `/seconds-contracts/orders` 拉取后按 status 与 symbol 过滤（`pc/src/api/second.ts:64-76`、`pc/src/api/second.ts:99-106`）。
2. UI 直接 `v-for="order in store.currentOrders"` 渲染活动订单表，每行展示方向、金额、开仓价、当前价、收益率和倒计时（`pc/src/views/SecondOptions.vue:467-509`）。这支持同一交易对同时存在多单；它不是跨交易对总览，因为 API load 使用当前 `symbol`。
3. 倒计时使用 `Record<orderId, seconds>`，每秒遍历所有当前订单更新（`pc/src/views/SecondOptions.vue:70-74`、`pc/src/views/SecondOptions.vue:218-230`）。到期结算查询用 `Set<orderId>` 防止同一订单重复并发查询，结算后按 ID 从数组移除（`pc/src/views/SecondOptions.vue:216-270`）。
4. PC 下单按钮只受提交中、周期选择和登录状态限制，不受已有活动订单限制（`pc/src/views/SecondOptions.vue:613-658`）。下单成功后刷新活动订单和余额（`pc/src/stores/second.ts:268-281`）。私有 WebSocket 收到 `seconds_contract.*` 或 `wallet.*` 事件时并行刷新余额、当前单和历史单（`pc/src/views/SecondOptions.vue:76-93`、`pc/src/views/SecondOptions.vue:324-331`）。
5. 当前 Mobile 只取第一个活动订单：`orders.find(...)`（`mobile/src/views/SecondsView.vue:66-83`）。方向、周期、金额快捷项和提交按钮都在 `Boolean(activeOrder)` 时禁用（`mobile/src/views/SecondsView.vue:559-638`、`mobile/src/views/SecondsView.vue:665-673`），形成“一次只能一单”的前端限制；活动态也只渲染一张卡（`mobile/src/views/SecondsView.vue:516-549`）。底部订单记录虽然来自整个数组，但只显示前三条（`mobile/src/views/SecondsView.vue:681-697`）。
6. 当前 Mobile 到期刷新锁是单个 `expiredReloadedOrderId`，定时器也只检查单一 `activeOrder`（`mobile/src/views/SecondsView.vue:57-64`、`mobile/src/views/SecondsView.vue:419-425`），无法独立处理多笔同批到期订单。

**可复用判断：**将 Mobile 的状态边界改为 `activeOrders = orders.filter(opened|pending|active)`；每笔订单按自身 symbol 获取 ticker，按自身 createdAt/expiresAt 计算倒计时与进度；展示层用 `v-for` 卡片/紧凑列表而非一张 active card；下单控件不再因为 `activeOrders.length > 0` 整体禁用。到期刷新去重可直接借鉴 PC 的 `Set<number>`，但避免 PC 同时结算时单一 modal 数据被后一个结果覆盖的问题。若产品要求跨交易对并行订单，列表不能像 PC 一样只按当前 symbol 拉取/展示。

### PC 与当前 Mobile 的关键差异

| Concern | PC reference | Current Mobile | Reuse decision |
| --- | --- | --- | --- |
| Header | 独立页内行情 Header，pair/logo/latest/24h change/high/low。 | 共享 60px `PageHeader` 只显示 pair；market board 仅 latest/周期/赔率。 | 复用字段层级，不复制桌面布局；保留 Mobile Header 几何。 |
| Tradable pairs | seconds products 为唯一来源，逐 symbol enrich ticker。 | seconds products 用于选择器，但 market ticker store 来自全市场。 | 保留产品源；ticker 订阅只覆盖产品 symbol。 |
| Live ticker | `/ws/seconds`，显式订阅所有秒合约 symbol。 | 秒合约页只 REST refresh，已有 WS 工具但未启动。 | 复用 Mobile `subscribeTickers`，不移植 PC socket URL。 |
| K 线 | 内部 REST + seconds WS；PC KLineCharts Pro 或 lightweight-charts。 | 只有一次 REST 微型 canvas 折线；无实时 candle。 | 复用 Mobile 已有本地 KLineCharts base renderer + stream/merge。 |
| Chart externality | 默认 KLineCharts；可选 lightweight renderer 含 TradingView 外链。 | 两个 npm 引擎均本地；现有测试禁止 iframe/script/anchor/remote data source。 | 秒合约默认/固定使用 Mobile KLineCharts 本地路径。 |
| Active orders | 当前 symbol 的 `currentOrders[]` 全量表格，逐 ID 倒计时，可继续下单。 | 单一 `activeOrder` 卡；有活动单时禁用全部下单控件。 | 改为 `activeOrders[]`，逐单状态，不再全局锁单。 |
| Settlement refresh | 每单 `Set` 防重轮询；私有事件刷新。 | 单一 expired ID，到期只刷新第一单；无页面私有事件订阅。 | 借鉴 per-order Set；私有事件需另行确认 Mobile 是否有连接合同。 |
| Guest | 页面 mount 和模板都整体挡在登录态之外。 | 产品和公共行情允许访客看，私有订单/钱包为空。 | 不复制 PC 的整页登录门禁。 |
| Adapter fidelity | 将 status 压成 OPEN/CLOSE，`active` 会落到 CLOSE；`closePrice` 固定 0。 | 保留后端 status、entry/settlement price，方向严格校验。 | 保留 Mobile adapter，禁止回退到 PC 映射。 |

### Recommended local reuse boundary

1. **Header/ticker data:** 参考 `SecondOptions.vue` 的 ticker 字段集合和 seconds-products-first symbol 集合；使用 Mobile `subscribeTickers()` 驱动现有 `MarketTicker`，并让 Header/market board 读取同一 reactive ticker。
2. **K 线 data owner:** 使用 `fetchKlines(symbol, '1m')` 首屏快照，加 Mobile 协议层的 `klineSubscriptionFrame`/`parseMarketSocketFrame`/`mergeMarketKlines`；沿用详情流的 generation、REST token、heartbeat、backoff 和 cleanup 模式。
3. **K 线 renderer:** 使用 `MobileMarketChart` 的 KLineCharts 分支，或直接使用 `KLineChartMarketChart`；保持 render-only，不引入 PC `KLineChartPro`、hosted widget、iframe、CDN 或 TradingView attribution anchor。
4. **多订单:** 订单源仍使用 Mobile `fetchSecondsOrders()` 和严格 `mapSecondsOrder()`；建立 `activeOrders[]`、逐单 ticker、逐单倒计时/进度、逐单 expiry refresh lock；下单提交锁只覆盖当前 mutation，不覆盖所有既有订单。
5. **Reconciliation:** `openSecondsOrder()` 返回值仍是提交成功边界，应先按 ID upsert，再把后续订单/钱包刷新当作 reconciliation；这比 PC 重复刷新更符合 Mobile spec。

### External references / versions

- 未访问外部文档或网络；本次按用户要求仅研究仓库代码与本地依赖清单。
- PC 本地依赖：`@klinecharts/pro ^0.1.1`、`klinecharts ^9.8.12`、`lightweight-charts ^5.2.0`（`pc/package.json:13-29`；lockfile 当前解析为 0.1.1 / 9.8.12 / 5.2.0）。
- Mobile 本地依赖：`klinecharts 10.0.0`、`lightweight-charts 5.2.0`（`mobile/package.json:23-29`）。本地测试锁定版本、内存 DataLoader、无外部 anchor/iframe/script/CDN/remote datafeed（`mobile/tests/market-detail-reference-layout.test.ts:205-281`）。

### Related specs

- `.trellis/spec/backend/seconds-contracts.md:130-188`：多周期产品、订单 symbol、seconds-products-first 交易对来源和 PC 适配合同。
- `.trellis/spec/backend/seconds-contracts.md:3-44`：活动/历史订单 created/expires 时间戳合同。
- `.trellis/spec/backend/realtime-websockets.md:3-68`：spot/margin/seconds 业务隔离 WebSocket、订阅 payload 和测试合同。
- `.trellis/spec/backend/platform-display-and-chart.md:25-47`：PC 图表使用内部 REST/WS 数据、默认 KLineCharts、禁止 hosted TradingView widget。
- `.trellis/spec/mobile/backend-integration.md:164-235`：Mobile WS、REST/WS 竞态、实时 K 线权威性、render-only 本地图表和版本合同。
- `.trellis/spec/mobile/backend-integration.md:239-251`：Seconds status/price 保真、利润公式和 create-response-first 提交边界。
- `.trellis/spec/mobile/index.md:178-184`：时间戳归一化、interval 来源和 live candle 优先合同。

## Caveats / Not Found

- `task.py current --source` 返回 `Current task: (none)`；研究输出路径由用户明确指定，因此写入该任务的 `research/`，未修改任务状态或其他文件。
- PC 的“多活动订单”仅覆盖当前选择交易对；`fetchSecondCurrentOrders(symbol)` 会在客户端过滤 symbol（`pc/src/api/second.ts:64-70`、`pc/src/api/second.ts:99-105`）。若 Mobile 需求是跨交易对并行展示，不能原样复制这一过滤边界。
- PC guest 测试名称声称保留公共行情，但实现实际在 mount 开头直接 return，模板也整页显示登录态（`pc/src/views/SecondOptions.vue:310-313`、`pc/src/views/SecondOptions.vue:348-352`）。该模式不应覆盖 Mobile 当前访客可看公共产品/行情的行为。
- PC `secondsStatusToPc()` 没有把 `active` 识别为 OPEN，且订单适配把 `closePrice` 固定为 0（`pc/src/api/backendAdapters.ts:1483-1503`、`pc/src/api/backendAdapters.ts:1781-1785`）；Mobile 的现有 adapter 更符合当前 spec。
- PC 历史 API 接受 page 参数但实际请求全量订单并在客户端过滤，store 再模拟分页；这不是可复用的分页参考（`pc/src/api/second.ts:74-76`、`pc/src/api/second.ts:99-106`；`pc/src/stores/second.ts:187-230`）。
- PC `TVChart.vue` 固定 dark theme、`en-US`、`Asia/Shanghai`，卸载时只取消订阅而未显式 dispose KLineChartPro（`pc/src/components/chart/TVChart.vue:49-67`、`pc/src/components/chart/TVChart.vue:146-149`）；不适合直接移植到 Mobile。
- 当前 Mobile `startMarketDetailStream()` 固定订阅 depth/trade/kline，没有只订阅 ticker+kline 的参数；这是复用时需要收敛的边界，不代表必须新建第二套解析/合并逻辑。
- 未找到 Mobile 私有 WebSocket 的现成 seconds/wallet 事件订阅实现；PC 的私有事件刷新只能作为模式参考。Mobile 可先依赖每单到期 reconciliation，或在后续明确私有 WS 合同后接入。
- 未运行测试或构建：本次是只读研究，唯一写入为本研究文档。
