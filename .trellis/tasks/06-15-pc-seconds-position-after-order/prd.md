# 修复 PC 秒合约下单后持仓不显示

## Goal

PC 秒合约下单成功后，当前委托/持仓列表应立即显示刚开的秒合约订单。

## What I already know

- PC 下单成功后会调用 `loadCurrentOrders(params.symbol)` 刷新当前委托。
- `fetchSecondOrders` 会按 `order.symbol` 和当前页面交易对过滤订单。
- 后端 `seconds_contract_orders` 查询目前只返回 `pair_id`，没有返回交易对 `symbol`。
- PC adapter 的 `BackendSecondsOrder.symbol` 是可选字段；当后端没返回时，过滤阶段已经把订单过滤掉。

## Requirements

- 用户端秒合约订单列表返回交易对 `symbol`。
- 下单成功返回的订单也应包含 `symbol`。
- 保持后台订单接口兼容，同时也可返回 `symbol` 和 `stake_asset_symbol`。
- PC 当前持仓过滤逻辑不需要绕过交易对过滤。

## Acceptance Criteria

- [ ] 秒合约下单成功响应里的 `order.symbol` 是真实交易对符号。
- [ ] `/seconds-contracts/orders` 用户列表中的订单带 `symbol`，PC 端能按当前交易对过滤并显示。
- [ ] 秒合约路由测试覆盖订单响应中的 `symbol`。
- [ ] PC adapter 测试保持通过。

## Out of Scope

- 不改秒合约撮合/结算逻辑。
- 不改页面视觉样式。
