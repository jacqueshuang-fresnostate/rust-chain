# 修复现货市价单参考价校验过严

## Goal

PC 端提交现货市价单时会带上 `reference_price` 用于后端滑点保护和钱包冻结金额计算。当前后端买入只要最新价高于参考价、卖出只要最新价低于参考价就直接拒单，导致行情轻微跳动时正常市价单失败。需要保留滑点保护，但允许小幅价格抖动。

## What I Already Know

- 用户复现请求：`POST /api/v1/spot/orders`，`pair_id=ETH-USDT`，`side=buy`，`order_type=market`，`quantity=1`，`reference_price=1717.8`。
- 当前报错来自 `src/modules/spot/routes.rs` 的 `ensure_market_price_within_reference`。
- `resolve_market_execution_price` 会优先读取 Redis ticker 的 `last_price`，没有 Redis 最新价时回退到请求里的 `reference_price`。
- 市价买入冻结 quote 资产时当前使用请求 `reference_price`；如果允许略高价格成交，需要同步用执行价冻结，否则成交金额可能超过冻结金额。

## Requirements

- 市价单仍然必须提供正数 `reference_price`。
- 市价买入允许执行价在参考价上方 10 bps（0.1%）以内；超过容差仍返回原有校验错误。
- 市价卖出允许执行价在参考价下方 10 bps（0.1%）以内；超过容差仍返回原有校验错误。
- 市价买入在执行价高于参考价但仍在容差内时，钱包冻结金额应按执行价计算，保证成交结算不会超过冻结金额。
- 不改变限价单触发逻辑。
- 不改变 PC 请求结构。

## Acceptance Criteria

- [ ] `reference_price=1717.8` 且最新价小幅高于参考价时，市价买入不再因为 `market price exceeds submitted reference price` 被拒。
- [ ] 买入超过 10 bps 容差仍被拒绝。
- [ ] 卖出低于 10 bps 容差仍被拒绝。
- [ ] 市价买入高于参考价但在容差内时，订单可以成交并正确扣减实际成交金额。
- [ ] 现有现货订单测试继续通过。

## Out of Scope

- 后台新增滑点配置页面。
- PC 端新增滑点设置。
- 改动撮合机器人、行情订阅或限价单成交策略。

## Technical Notes

- 关键文件：`src/modules/spot/routes.rs`，`tests/spot_routes.rs`。
- 需要用 Redis ticker 路径补集成测试，因为无 Redis 时后端会直接回退到请求参考价，无法复现该 bug。
