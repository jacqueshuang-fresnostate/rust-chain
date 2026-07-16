# PC现货页WS订阅实时更新修复

## 背景

`/spot/BTC_USDT` 现货交易页依赖公共 WebSocket 获取实时 ticker、depth、trade、kline 数据。当前订阅层只识别少数直接 payload 字段，并且行情 store 更新 ticker 时按展示符号做精确匹配，容易在 `BTCUSDT`、`BTC/USDT`、`BTC-USDT` 等格式之间失配，导致页面部分行情不实时刷新。

## 目标

- 让 PC 现货页的 ticker、盘口、成交、K 线 WS 消息能稳定路由到对应订阅。
- 同一交易对的不同符号格式应命中同一条 store 数据，不产生重复 ticker。
- 保持现有后端 `/ws/public` 多订阅协议兼容，不扩大到秒合约或闪兑通道。

## 范围

- 前端公共 WS 适配层：`pc/src/api/stomp.ts`
- PC 行情 store：`pc/src/stores/market.ts`
- WS 订阅回归测试：`pc/tests/stomp.test.ts`

## 验收标准

- WebSocket 接收到 direct payload、常见 envelope payload 时，都能识别 channel、symbol、interval。
- `/spot/BTC_USDT` 对应的 `BTC/USDT` 页面能用 `BTCUSDT` 或 `BTC-USDT` 的 ticker payload 更新当前行情。
- depth/trade/kline 订阅不会因为符号格式不同而丢消息。
- 添加测试覆盖以上行为，并通过最贴近变更的测试命令。

## 非目标

- 不重构交易页整体布局。
- 不修改后端 WS 协议。
- 不调整秒合约、闪兑或合约订阅逻辑。
