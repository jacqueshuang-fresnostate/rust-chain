# Ego 行情对比记录

## 结论

HIPPO 手机端 `/trade/BTC_USDT?mode=contract` 当前展示的不是 Bitget `BTCUSDT` USDT 永续合约行情，而是 Bitget `BTCUSDT` **现货**行情。差异来自交易品种口径，不是单纯的行情延迟、小数位或前端格式化问题。

## 同时段证据

采集时间：2026-08-12 04:26 HKT（Ego 任务空间 `compare bitget futures and hippo`）。

| 来源 | 最新价 / 买一价 | 24h 最高 | 24h 最低 | 口径 |
| --- | ---: | ---: | ---: | --- |
| HIPPO 合约页订单簿中间价 | `63,635.51` | `64,493.51` | `63,235.29` | 页面宣称“永续合约” |
| HIPPO `GET /api/v1/markets/BTCUSDT/ticker` | `63,635.00` | `64,493.51` | `63,235.29` | 缓存 ticker |
| Bitget 现货 REST | 最新价 `63,635.00`，买一 `63,635.51` | `64,493.51` | `63,235.29` | `api/v2/spot/market/tickers` |
| Bitget USDT 永续 REST | `63,615.60` | `64,467.50` | `63,218.40` | `api/v2/mix/market/ticker` |
| Bitget USDT 永续官网 | `63,602.60` | `64,467.50` | `63,218.40` | 官网页面在数秒后读取 |

HIPPO API 的 `observed_at=1786480006253`，Bitget 现货帧 `ts=1786480005951`，仅相差约 302 ms；因此这次观测中 Redis ticker 是新鲜的，可排除“项目重启后旧 Redis 值冒充实时行情”。

HIPPO 页面的中间价与 Bitget 现货买一价完全相同，HIPPO ticker 的最新价、24h 高低价和成交量也与 Bitget 现货 REST 逐字段相同。这组对照可以唯一解释当前价差。

## 代码链路

1. 后端 Bitget WebSocket 订阅在 `src/modules/market/infrastructure.rs:1902-1907` 固定使用 `instType: "SPOT"`，ticker、深度、成交和 K 线都是现货频道。
2. 后端 REST 兜底在 `src/modules/market/infrastructure.rs:1657-1659` 固定请求 `/api/v2/spot/market/tickers`。
3. 手机端合约模式仍在 `mobile/src/views/TradeView.vue:223-230` 调用通用 `fetchKlines` / `fetchOrderBook` / `fetchRecentTrades`，没有传入合约行情口径。
4. 上述 API 在 `mobile/src/api/market.ts:77-106` 只请求 `/markets/:symbol/{klines,depth,trades}`，现货和合约模式共用同一条数据链。
5. `TradeView.vue:111,131` 的 ticker 与当前价也来自同一个全局现货 `marketStore`。

## 修复边界

不应把全局 `market:ticker:{symbol}` 直接改成合约行情，因为现货下单触发、闪兑、秒合约、资产估值和风控都在使用这个现货口径。正确改法是建立独立的 `USDT-FUTURES` 行情链：

- 后端独立订阅 Bitget `instType=USDT-FUTURES`。
- ticker / depth / trade / kline 使用独立 Redis/Mongo 命名空间，并提供独立 REST 和 WebSocket 频道。
- 合约 ticker 补充 `markPrice`、`indexPrice`、`fundingRate`，不用现货 `lastPr` 伪装标记价。
- 手机端 `mode=contract` 只订阅合约频道；`mode=spot` 保持现有现货链。
- 结算、强平与触发价应在产品规则明确后再切换，避免只修 UI 却造成展示价和执行价不一致。

## 本次改动决策

本次完成行情口径诊断与证据固化，未直接改动生产行情或订单结算逻辑。原因是当前问题需要新增一套合约行情边界，而不是安全的单字段替换；直接把 SPOT 改为 USDT-FUTURES 会同时改变现货和其他资产业务的价格口径。
