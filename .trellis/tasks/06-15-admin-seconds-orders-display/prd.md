# 后台秒合约订单列表显示优化

## Goal

后台秒合约订单列表需要更适合运营查看：显示用户邮箱、交易对、结算价格，并隐藏订单 ID、用户 ID、产品 ID。

## Requirements

- 后台秒合约订单列表返回并展示用户邮箱。
- 后台秒合约订单列表展示交易对 `symbol`。
- 秒合约订单记录结算价格快照，自动结算时写入行情退出价。
- 后台秒合约订单列表展示结算价格；未结算订单为空。
- 后台表格不显示 `订单ID`、`用户ID`、`产品ID` 列。

## Acceptance Criteria

- [x] `GET /admin/api/v1/seconds-contracts/orders` 返回 `email`、`symbol`、`settlement_price`。
- [x] 自动结算后的订单保存 `settlement_price`。
- [x] 后台秒合约订单表格显示邮箱、交易对、结算价格。
- [x] 后台秒合约订单表格不显示订单 ID、用户 ID、产品 ID。
- [x] 后端和后台测试覆盖新增字段与列配置。

## Out of Scope

- 不调整秒合约盈亏算法。
- 不改人工结算弹窗结构；人工结算没有行情价格时结算价格可为空。
