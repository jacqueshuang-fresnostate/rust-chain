# 秒合约历史盈亏数据流审计

## 当前数据流

`seconds_contract_orders` 的本金、赔率与结果快照 → `GET /api/v1/seconds-contracts/orders` → `mapSecondsOrder()` → `SecondsHistoryView.vue`。

## 已确认事实

- 订单响应已包含 `stake_amount`、`payout_rate`、`result` 和 `stake_asset_symbol`，手机端无需读取实时行情或当前产品配置即可表达历史盈亏。
- 后端结算把 `payout_rate` 定义为净收益率：赢单总入账为 `stake + stake × payoutRate`，因此历史“盈利金额”是 `stake × payoutRate`，不是总派彩。
- 输单结算入账为零，且本金已在开仓时扣除，因此历史“亏损金额”是 `-stake`。
- 取消、缺少结果和未来未知结果没有权威盈亏结论，展示 `--` 比伪造零值更准确。
- `secondsOrderEstimatedProfit()` 已是活动订单预计净收益的唯一前端口径，应复用而不是复制公式。

## 边界与风险

- 盈亏模型必须只用于展示，不能回流到下单或钱包请求。
- 结果比较应兼容后端大小写与首尾空白，但不得把未知值强制映射为输赢。
- 金额单位必须来自订单快照的 `stakeAssetSymbol`。
- 新增完整宽度盈亏行不得破坏 320px 两列明细布局。
