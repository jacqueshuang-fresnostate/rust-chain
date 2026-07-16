# PC端交易页面接口与WSS对接审计

审计时间：2026-06-16 01:47

## 总结

- 现货 `/spot/:symbol?`：HTTP 与公共行情 WSS 基本已对接，订单创建、撤单、当前委托、历史委托、钱包、盘口、成交、K 线都有对应 Rust 后端接口。
- 合约 `/contract/:symbol?`：行情 HTTP/WSS 已接，交易侧只有“开仓为保证金仓位”和“整仓平仓”接到 `/margin/positions`；页面上的限价委托、撤单、全部平仓、划转、修改杠杆等控件仍未接到后端能力，且开仓参数语义存在明显不匹配。
- 秒合约 `/second/:symbol?`：产品、周期、下单、余额、当前/历史订单、ticker 与 K 线已接；历史记录的结算价未映射到页面，分页参数未下发，订单结算依赖轮询全量订单过滤，没有接私有 WSS 推送。
- WSS：PC 已拆成 `spot`、`margin`、`seconds` 三个业务 client，但三者当前都连接同一个 `/ws/public`；这能隔离前端订阅状态，但后端仍是统一公共行情 WS，不是业务独立 WS 端点。
- 私有 WSS：后端已经发布用户私有事件，例如现货订单、保证金仓位、秒合约订单，但 PC 端交易页没有订阅 `/ws/private`，因此订单/持仓状态主要靠提交后的主动刷新和秒合约倒计时轮询。

## 现货页面 `/spot/:symbol?`

### 已对接

- 页面路由指向 `Trade.vue`。
- 初始行情：
  - `fetchMarketSnapshot()` -> `GET /markets`
  - 每个市场补 ticker -> `GET /markets/:symbol/ticker`
- 盘口：
  - REST：`fetchTradePlate()` -> `GET /markets/:symbol/depth`
  - WSS：`stompService.subscribe('spot', 'spot:depth:{symbol}')`
- K 线：
  - REST：`TVChart` 默认 spot fetcher -> `GET /markets/:symbol/klines`
  - WSS：`spot:kline:{symbol}:{interval}`
- 最新成交：
  - REST：`MarketTrades` 默认 spot fetcher -> `GET /markets/:symbol/trades`
  - WSS：`spot:trade:{symbol}`
- 下单/撤单/订单列表：
  - `POST /spot/orders`
  - `DELETE /spot/orders/:id`
  - `GET /spot/orders?pair_id=&status=&limit=`
- 钱包：
  - `GET /wallet/accounts`

### 风险

- 当前没有订阅 `/ws/private`，所以订单被撮合、取消、拒绝等状态不会通过私有 WS 主动推到当前/历史委托；现在主要靠下单/撤单后的 `triggerOrderRefresh()` 和用户切换 tab/页面刷新。
- K 线 WSS 前端订阅已存在，但是否有实时推送取决于行情 worker 是否启用对应 provider/channel 的 kline 广播。

## 合约/杠杆页面 `/contract/:symbol?`

### 已对接

- 页面交易对来自 `GET /margin/products`，路由里不存在的交易对会回退到第一个已配置合约产品。
- 行情仍复用公共市场数据：
  - 盘口 REST：`GET /markets/:symbol/depth`
  - 盘口 WSS：`margin:depth:{symbol}`
  - ticker WSS：`margin:ticker:{symbol}`
  - K 线 REST/WSS：`margin:kline:{symbol}:{interval}`
  - 成交 REST/WSS：`margin:trade:{symbol}`
- 开仓调用：
  - `openPosition()` -> `POST /margin/positions`
- 平仓调用：
  - `closePosition()` -> 先查 `GET /margin/positions?status=opened`，再 `POST /margin/positions/:id/close`
- 持仓/历史：
  - `GET /margin/positions`
  - 前端把 positions 映射成持仓、当前委托和历史委托。

### 未对接或半对接

- 修改杠杆：页面 `confirmLeverage()` 调用 `contractStore.setLeverage()`，最终走 `modifyLeverage()`；该函数直接 `Promise.reject`，后端也没有用户级修改杠杆接口。
- 划转：页面 `confirmTransfer()` 调用 `contractStore.transfer()`，最终走 `transferFunds()`；该函数直接 `Promise.reject`，后端没有 margin 钱包划转接口。
- 撤单/全部撤单：`cancelOrder()`、`cancelAllOrders()` 都是 `Promise.reject`。当前后端 margin 也没有“保证金委托单”实体，只有仓位。
- 全部平仓：页面 `submitCloseAll()` 调用 `closeAllPositions()`，该函数直接 `Promise.reject`。
- 保证金模式切换：store 有 `setMarginMode()`，底层 `canSwitchPattern()`、`switchPattern()` 都是 `Promise.reject`；页面当前的 `confirmMarginMode()` 只改本地下单参数，并显示成功，不会持久化账户级模式。
- 限价/市价语义不匹配：UI 有限价输入和 `entrustPrice`，但 `mapPcMarginOpenRequest()` 发给后端时只发送 `product_id`、`direction`、`margin_mode`、`margin_amount`、`leverage`，没有价格或订单类型。后端开仓按缓存 ticker 立即创建仓位，不是限价委托。
- 数量语义不匹配：UI 的合约下单金额输入显示为基础资产数量，并计算 `cost = price * amount / leverage`，但 adapter 把 `amount` 原样作为 `margin_amount` 发给后端。后端 `margin_amount` 是保证金资产金额。
- 部分平仓/限价平仓未接：页面可输入平仓数量和限价，但 `closePosition()` 只按方向找到一个 open position 并调用 `/close`，没有传数量、价格或类型；后端也是整仓按缓存 ticker 平仓。
- 当前委托/历史委托不是独立委托数据：`fetchCurrentOrders()`/`fetchHistoryOrders()` 读的是 `/margin/positions`，把 opened/closed positions 映射成订单行；因此当前委托实际上不是待成交委托。

## 秒合约页面 `/second/:symbol?`

### 已对接

- 产品交易对和周期：
  - `GET /seconds-contracts/products`
  - 前端按 `products[].cycles` 展示每个周期的赔率、最小押注、最大押注。
- ticker：
  - 初始 ticker 从秒合约产品对应市场补 `GET /markets/:symbol/ticker`
  - WSS：为每个秒合约交易对订阅 `seconds:ticker:{symbol}`
- K 线：
  - `TVChart module="seconds"` 复用 `GET /markets/:symbol/klines`
  - WSS：`seconds:kline:{symbol}:{interval}`
- 下单：
  - `POST /seconds-contracts/orders`
  - 请求包含 `product_id`、`duration_seconds`、`direction`、`stake_amount`、`idempotency_key`
- 当前/历史订单：
  - `GET /seconds-contracts/orders`
  - 前端按 status 和 symbol 过滤。
- 余额：
  - `GET /wallet/accounts`，取 USDT 可用余额。

### 未对接或半对接

- 历史记录结算价未显示：后端订单返回 `settlement_price`，但 `mapSecondsOrdersToPcOrders()` 里 `closePrice` 固定为 `0`。
- 分页参数未下发：`fetchSecondHistoryOrders({ pageNo, pageSize })` 最终仍是 `GET /seconds-contracts/orders`，没有带 `limit`，也没有真正按页请求；无限滚动只是在前端基于返回数量判断。
- 订单结果查询效率低：`fetchSecondOrderResult(id, symbol)` 每次都请求全量 `/seconds-contracts/orders` 后本地过滤。
- 私有 WS 未接：后端会发布 `seconds_contract.order.opened/settled` 私有事件，但 PC 页面没有订阅 `/ws/private`；当前靠倒计时结束后轮询结果。
- 秒合约划转：store 暴露 `transfer()`，底层 `transferSecondFunds()` 直接 `Promise.reject`。主页面已没有划转入口，因此是未使用的残留能力。
- 旧页面 `BinaryOptions.vue` 不在路由中，但文件仍存在，使用简化倒计时和 `api/option.ts`；它不是当前 `/second/:symbol?` 页面，后续可考虑删除或归档，避免误用。

## WSS现状

- `stompService` 维护 `spot`、`margin`、`seconds` 三个独立 client 状态，调用层已经按页面分开。
- `endpointPath()` 目前无论业务类型都返回 `/ws/public`。
- 前端发送统一指令：
  - `{ op: "subscribe", channel, symbol, interval? }`
- 后端 `/ws/public` 支持 `ticker`、`depth`、`trade`、`kline` 四类公共行情频道。
- PC 端没有私有事件 client，交易状态、持仓、结算、成交回报没有实时用户级推送链路。

## 建议修复顺序

1. 合约页先处理参数语义：明确后端是“保证金仓位”还是“合约委托”，把 UI 的数量、保证金、杠杆、限价/市价改到与后端一致。
2. 合约页隐藏或改造未支持功能：撤单、全部平仓、划转、修改杠杆、限价平仓，避免用户点击后只得到失败 toast。
3. 秒合约补 `settlement_price -> closePrice` 映射，并让订单列表接口支持 status、symbol、limit/offset 或 page/pageSize。
4. PC 新增 `/ws/private` client，订阅用户订单/持仓/秒合约结算事件，用于刷新当前委托、历史委托、持仓和秒合约结果。
5. 如要真正做到业务 WSS 独立，再在后端增加 `/ws/spot`、`/ws/margin`、`/ws/seconds` 或至少在消息 envelope 中加入业务 namespace；当前只是前端 client 状态隔离。
