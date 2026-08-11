# 核对 Bitget 永续与前端行情偏差

## Goal

使用 Ego 浏览器在同一时间窗口对比 Bitget `BTCUSDT` USDT 永续页面、HIPPO 前端可见价格与后端公开 ticker，定位价格不一致来自交易品种口径、行情 Provider、Redis 新鲜度、接口映射或前端 WebSocket/缓存链路，并修复能够由当前代码确认的问题。

## Requirements

- 通过 Ego 浏览器访问用户指定的 Bitget USDT 永续页面，提取页面交易品种和最新成交价。
- 调试当前可运行的 HIPPO 前端，记录页面价格、请求接口、WebSocket 消息和更新时间。
- 同时读取 HIPPO 公开 ticker，核对 `symbol`、`last_price`、`observed_at` 与实际 Provider。
- 明确区分 Bitget 现货 `BTCUSDT`、USDT 永续 `BTCUSDT` 和标记价/指数价口径。
- 若确认存在前端或后端实现缺陷，在不改变无关交易逻辑的前提下修复并增加回归测试；若属于品种口径差异，给出可执行的接入边界和配置结论。

## Acceptance Criteria

- [x] 保存同一调试时段的 Bitget 永续、HIPPO 页面和 HIPPO API 价格证据。
- [x] 确认前端展示价格最终来自哪个 REST/WS 字段及交易品种。
- [x] 确认线上行情是否新鲜，并排除旧 Redis ticker 冒充实时数据。
- [x] 根因结论能解释观测到的价差，而不是只列举可能原因。
- [x] 如有代码改动，聚焦测试、类型检查/构建和差异检查通过；如无代码改动，明确记录原因和浏览器验证结果。

## Out of Scope

- 不在未确认需求时把现货交易、合约交易和秒合约合并为同一价格口径。
- 不修改 Bitget 官网或伪造其页面数据。
- 不调整订单结算价格口径，除非本次证据确认当前实现违反既有合同。

## Technical Notes

- 用户指定页面：`https://www.bitget.com/zh-CN/futures/usdt/BTCUSDT`。
- 现有行情 Provider 仅包含 Bitget、HTX、Coinbase；Bitget adapter 当前订阅 `instType=SPOT`。
- HIPPO 公开 ticker：`GET /api/v1/markets/:symbol/ticker`，数据来自 Redis `market:ticker:{symbol}`。
- 重点检查：`src/modules/market/infrastructure.rs`、`src/workers/market_feed.rs`、`mobile/src/api/market.ts`、`mobile/src/stores/market.ts` 及交易页面。
