# 修复竞猜下注报价和资产选择

## Goal

修复 PC 竞猜详情页无法下注的问题，确保报价请求和下单请求带用户登录态；下注面板显示用户可用余额；下注资产使用带 logo 的可搜索下拉选择；报价快照在报价成功后能展示真实数据。

## Requirements

* `/api/v1/prediction/quotes` 仍然是用户登录态接口，PC 请求必须正确携带 Bearer token，避免 401。
* 如果 access token 过期，PC 请求层需要先通过 `/auth/refresh` 刷新用户 token 并重试原请求，refresh 失败时才清理登录态。
* 竞猜下注面板需要展示当前选择资产的用户余额。
* 下注资产选择需要显示资产 logo、符号，并支持搜索。
* 报价成功后显示报价快照，包括价格、手续费、份额、最大返还/潜在返还和过期时间等关键字段。
* 下单必须消费后端 quote，不能前端自行计算后直接下单。
* 保持现有 prediction API、订单展示和本地化结构，不扩大到结算/后台配置重构。

## Acceptance Criteria

* [x] PC prediction API 的 quote/order 请求使用统一 `request.instance` 并带 Authorization。
* [x] PC 请求层在用户接口 401 时先刷新 access token 并重试，避免 quote 接口因短期 token 过期直接失败。
* [x] 详情页下注面板可以看到余额和带 logo 的资产下拉搜索。
* [x] 报价快照在 quote 成功后显示数据，失败时给出明确提示。
* [x] 点击下注时如未登录跳转登录，已登录则先报价再下单。
* [x] PC type-check 与 prediction 相关测试通过。

## Definition of Done

* 相关 PC 测试和类型检查通过。
* 如触碰后端，执行最贴近的 Rust 测试/检查。
* `docs/superpowers/PROGRESS.md` 记录本次交付。

## Technical Notes

* 相关文件预计包括：`pc/src/api/request.ts`、`pc/src/api/prediction.ts`、`pc/src/views/Prediction.vue`、`pc/src/api/backendAdapters.ts`、`pc/tests/prediction-page-routing.test.ts` 或相关静态测试。
* 后端合同参照 `.trellis/spec/backend/prediction-markets.md`：quote 必须绑定用户并由订单消费。
