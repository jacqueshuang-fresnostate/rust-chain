# 重新核对 Bitget 现货与手机端行情

## Goal

纠正上一次误将 Bitget 永续页与手机端合约页作为对比对象的调试偏差，使用 Ego 在同一时间窗口比较 Bitget `BTCUSDT` 现货页、Bitget 现货 REST、HIPPO 手机端现货页及 HIPPO REST/WS，判断最新价、订单簿和最新成交是否对应。

## Requirements

- 对比 Bitget 现货页 `https://www.bitget.com/zh-CN/spot/BTCUSDT`，不再使用 futures URL。
- 调试 HIPPO 现货路由 `/trade/BTC_USDT`，不带 `mode=contract`。
- 同时读取 Bitget 现货 ticker REST 和 HIPPO 公开 ticker，对比时间戳、最新价、24h 高低和成交量。
- 比较双方现货订单簿买一/卖一，并确认 HIPPO 页面取值是最新成交价还是最优买卖价。
- 检查最新成交与 K 线的 REST/WS 链路，必须区分 Bitget 外部成交与 HIPPO 内部现货成交。
- 手机端首页行情列表同样必须以 Bitget 现货 ticker 最新成交价为权威价格，并随后端实时 ticker WebSocket 更新。
- 交易页与首页使用同一 ticker 口径；订单簿买一/卖一、内部历史成交和 K 线收盘价只作各自组件数据，不得覆盖页面权威最新价。
- 如果发现可确认的前端取值或映射缺陷，在不改变交易价口径的前提下修复；如果数据已经对应，保存实测数据与显示差异解释。

## Acceptance Criteria

- [x] Ego 同时段证据只涉及 Bitget 现货与 HIPPO 现货。
- [x] 最新价、买一/卖一、24h 高低、ticker 时间戳完成定量比对。
- [x] 明确 HIPPO 现货页主价格的实际取值优先级。
- [x] 明确订单簿、最新成交和 K 线是否都来自 Bitget 现货实时帧。
- [x] 首页 BTC/USDT 价格与同时段 Bitget 现货 ticker 对应，且实时帧后续更新可见。
- [x] 交易页主价格与首页价格共用 ticker 权威口径，不被内部历史成交或旧 K 线覆盖。
- [x] 根因结论可复现，并记录到任务 research 与项目进度文件。

## Out of Scope

- 不对比 Bitget USDT 永续、标记价或指数价。
- 不把 HIPPO 内部用户成交冒充为 Bitget 全市场最新成交。
- 不修改无关合约、秒合约或结算逻辑。

## Technical Notes

- 上次错误对比打开了 Bitget `/futures/usdt/BTCUSDT` 与 HIPPO `?mode=contract`；本次两端都必须使用现货路由。
- 后端 Bitget provider 已确认订阅 `instType=SPOT`，需重点检查前端主价格优先级、订单簿排序、成交初始 REST 和 WS 增量口径。
