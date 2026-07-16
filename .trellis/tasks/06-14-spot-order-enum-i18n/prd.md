# 现货订单类型和方向多语言

## Goal

PC 端现货交易页的“当前委托”和“历史委托”表格不应直接展示 `LIMIT_PRICE`、`MARKET_PRICE`、`BUY`、`SELL` 等内部枚举码，而应按当前语言显示给用户可理解的订单类型和方向。

## Requirements

- 在现货订单表格中，将订单类型按 i18n 显示：
  - `LIMIT_PRICE` -> `trade.order_type_limit_price`
  - `MARKET_PRICE` -> `trade.order_type_market_price`
  - 兼容小写/后端原始值：`limit`、`market`
- 在现货订单表格中，将方向按 i18n 显示：
  - `BUY` -> `trade.order_side_buy`
  - `SELL` -> `trade.order_side_sell`
  - 兼容小写：`buy`、`sell`
- 撤单确认弹窗里的方向也使用同一套 i18n 显示。
- 未识别的类型/方向保留原值，避免隐藏异常数据。

## Acceptance Criteria

- [ ] 当前委托表格不再直接显示 `LIMIT_PRICE`、`MARKET_PRICE`、`BUY`、`SELL`。
- [ ] 历史委托表格不再直接显示 `LIMIT_PRICE`、`MARKET_PRICE`、`BUY`、`SELL`。
- [ ] 中文环境显示“限价单 / 市价单 / 买入 / 卖出”。
- [ ] 英文环境显示“Limit / Market / Buy / Sell”。
- [ ] 现货下单请求和后端适配器仍保留原枚举码，不改变接口契约。

## Definition of Done

- PC 前端类型检查通过。
- 触碰文件空白检查通过。
- 更新 `docs/superpowers/PROGRESS.md`。

## Technical Approach

- 修改 `pc/src/components/trade/OrderHistory.vue`，新增展示层格式化函数。
- 修改 `pc/src/i18n/index.ts`，在 `en.trade` 和 `zh.trade` 下补充订单类型/方向文案。
- 不修改 `pc/src/api/backendAdapters.ts` 的枚举映射，避免影响下单和订单接口契约。

## Out of Scope

- 不处理订单状态 i18n。
- 不处理合约/秒合约订单页面。
- 不重构订单表格结构和样式。

## Technical Notes

- `pc/src/components/trade/OrderHistory.vue` 当前直接渲染 `order.type` 和 `order.direction`。
- `pc/tests/backendAdapters.test.ts` 断言适配器输出 `LIMIT_PRICE`、`BUY` 等内部码，这些测试不应改为中文。
