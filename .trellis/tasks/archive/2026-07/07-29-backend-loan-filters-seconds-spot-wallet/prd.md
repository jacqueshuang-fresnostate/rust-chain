# 完成贷款产品筛选并统一秒合约现货钱包语义

## Goal

让后台贷款产品列表的贷款类型与状态筛选真正作用于服务端查询和分页总数；同时明确秒合约不建立独立资金账户、不提供资金划转，统一直接使用现货钱包余额完成下单扣款和结算入账。

## What I Already Know

- 后台贷款产品页面已经发送 `loan_type`、`status`、`limit` 和 `offset`。
- `AdminLoanProductsQuery` 当前只接收 `limit`、`offset`，应用层固定向仓储传入 `status = None`。
- 贷款产品行查询与总数查询已经共用 `push_loan_product_filters`，适合扩展为同时处理贷款类型和状态。
- 秒合约下单当前直接锁定并扣减 `wallet_accounts.available`，盈利结算也直接回写同一个 `wallet_accounts` 账户。
- PC 秒合约页面已经移除划转入口，但 `pc/src/api/second.ts` 和 `pc/src/stores/second.ts` 仍保留不可用的划转类型与方法。

## Requirements

- `GET /admin/api/v1/loan/products` 支持可选 `loan_type` 和 `status` 查询参数。
- 空字符串筛选按未筛选处理；非空筛选必须经过现有贷款类型和产品状态规则校验。
- 行查询与 `total` 计数必须使用完全相同的筛选条件。
- 任意单筛选和组合筛选均返回正确结果。
- 秒合约继续直接使用共享 `wallet_accounts` 现货钱包，不新增秒合约钱包表或划转路由。
- 删除 PC 秒合约 API/Store 中不可用的划转类型、方法和导出，页面保持无划转入口。
- 更新秒合约后端规范，明确共享现货钱包和禁止独立划转的契约。

## Acceptance Criteria

- [x] 按 `loan_type` 筛选贷款产品时只返回匹配产品，`total` 与结果一致。
- [x] 按 `status` 筛选贷款产品时只返回匹配产品，`total` 与结果一致。
- [x] 同时传入 `loan_type` 和 `status` 时按 AND 组合筛选。
- [x] 非法贷款类型或状态返回校验错误，不进入数据库查询。
- [x] PC 秒合约源码不再暴露 `SecondTransferParams`、`transferSecondFunds` 或 Store `transfer`。
- [x] 秒合约下单扣减、结算入账继续使用 `wallet_accounts`，相关测试通过。
- [x] Rust 格式、编译、聚焦测试、PC 类型检查和聚焦测试通过。

## Out Of Scope

- 不实现贷款逾期清算或罚息计提。
- 不新增秒合约资金划转接口。
- 不新增秒合约独立钱包或资金账户。
- 不修改秒合约下单、赔率、结算公式或现有 UI 布局。

## Technical Notes

- 后端涉及 `src/modules/loan/{presentation,application,infrastructure}.rs`。
- 秒合约资金语义涉及 `src/modules/seconds_contract/application.rs`、`src/workers/seconds_contract_settlement.rs`、`pc/src/api/second.ts` 和 `pc/src/stores/second.ts`。
- 集成验证优先扩展 `tests/loan_routes.rs`；PC 使用现有秒合约静态/适配测试。

## Definition Of Done

- 后端和 PC 改动完成并通过聚焦验证。
- 更新 `.trellis/spec/backend/seconds-contracts.md` 与任务上下文。
- 更新 `docs/superpowers/PROGRESS.md`。
- 改动提交到当前分支。
