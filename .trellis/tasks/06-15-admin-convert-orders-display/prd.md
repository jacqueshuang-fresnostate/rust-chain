# 后台闪兑订单列表显示优化

## Goal

后台“闪兑订单”列表不再显示内部关联字段 `quote_id`、`user_id`、`convert_pair_id`，改为展示运营可读的源资产、目标资产和用户邮箱。

## What I Already Know

- 后台路由为 `GET /admin/api/v1/convert/orders` 和 `GET /admin/api/v1/convert/orders/:id`。
- 当前后端响应 `ConvertOrderResponse` 只有 `from_asset_id/to_asset_id/user_id`，没有资产符号和用户邮箱。
- 当前后台资源表 `resourceConfigs.convertOrders` 显示了“报价ID / 用户ID / 交易对ID”列。
- 订单行级“查看详情”依赖订单 `id`，所以订单 ID 本次保留。

## Requirements

- 后台闪兑订单接口返回 `from_asset_symbol`、`to_asset_symbol`、`user_email`。
- 后台闪兑订单表格移除“报价ID / 用户ID / 交易对ID”列。
- 后台闪兑订单表格新增“用户邮箱 / 源资产 / 目标资产”列。
- 保持现有用户 ID、邮箱、状态、limit 筛选能力不变。
- 不改变用户端 `/api/v1/convert/orders` 响应。

## Acceptance Criteria

- [ ] `/admin/api/v1/convert/orders` 返回的订单包含用户邮箱、源资产符号、目标资产符号。
- [ ] 后台表格配置不包含 `quote_id`、`user_id`、`convert_pair_id` 三个列。
- [ ] 后台表格配置包含 `user_email`、`from_asset_symbol`、`to_asset_symbol` 三个列。
- [ ] 现有查看详情按钮仍通过订单 `id` 正常请求详情。
- [ ] 后端和前端测试覆盖字段/列变化。

## Out Of Scope

- 不删除数据库字段。
- 不改变闪兑订单创建、确认、结算逻辑。
- 不改变用户端个人中心闪兑订单展示。

## Technical Notes

- 后端位置：`src/modules/admin/routes.rs`
- 前端位置：`web/src/admin/resources/resourceConfigs.tsx`
- 后端测试：`tests/admin_routes.rs`
- 前端测试：`web/src/admin/resources/resourceConfigs.test.tsx`
