# 现货订单状态多语言

## Goal

PC 端现货交易页的“当前委托”和“历史委托”表格状态列不应直接展示 `TRADING`、`SUBMITTED`、`CANCELED`、`COMPLETED` 等内部状态码，而应按当前语言显示用户可理解的状态。

## Requirements

- 在现货订单表格状态列中，将状态按 i18n 显示。
- 覆盖当前 PC 适配器会输出的状态：
  - `TRADING`
  - `CANCELED`
  - `COMPLETED`
  - `REJECTED`
- 兼容组件已有撤单判断使用的 `SUBMITTED`。
- 兼容后端原始状态或拼写差异：
  - `open`
  - `pending`
  - `partially_filled`
  - `filled`
  - `cancelled`
  - `canceled`
  - `failed`
  - `expired`
- 未识别的状态保留原值，避免隐藏异常数据。
- 不改变现货订单接口、适配器枚举值和撤单判断逻辑。

## Acceptance Criteria

- [ ] 当前委托表格状态列不再直接显示常见内部状态码。
- [ ] 历史委托表格状态列不再直接显示常见内部状态码。
- [ ] 中文环境显示“委托中 / 已提交 / 待处理 / 部分成交 / 已成交 / 已撤销 / 已拒绝 / 失败 / 已过期”等文案。
- [ ] 英文环境显示“Open / Submitted / Pending / Partially Filled / Filled / Canceled / Rejected / Failed / Expired”等文案。
- [ ] 现货下单、订单查询适配器和撤单按钮可见性不受影响。

## Definition of Done

- PC 前端类型检查通过。
- 触碰文件空白检查通过。
- 更新 `docs/superpowers/PROGRESS.md`。

## Technical Approach

- 修改 `pc/src/components/trade/OrderHistory.vue`，新增 `formatOrderStatus` 展示层函数。
- 修改 `pc/src/i18n/index.ts`，在 `en.trade` 和 `zh.trade` 下补充订单状态文案。
- 沿用上一任务中的 `normalizeOrderEnum` 和 unknown fallback 方式。

## Out of Scope

- 不处理订单类型和方向以外的新展示列。
- 不处理合约、秒合约或理财订单状态。
- 不改后端状态枚举和 PC 适配器映射。

## Technical Notes

- `pc/src/api/backendAdapters.ts` 中 `spotOrderStatusToPc` 会输出 `TRADING`、`CANCELED`、`COMPLETED`、`REJECTED`。
- `pc/src/components/trade/OrderHistory.vue` 当前撤单按钮仍根据 `TRADING` / `SUBMITTED` 判断，不能因展示翻译而改变。
