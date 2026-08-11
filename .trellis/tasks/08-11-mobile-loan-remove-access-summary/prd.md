# 移除手机借贷账户摘要

## Goal

移除借贷页中冗余的 `loan-access-pencil__summary` 账户登录摘要，让产品列表更紧凑；未登录用户仍保留明确的登录查看额度按钮。

## Requirements

- 从 `LoanView` 模板中删除 `loan-access-pencil__summary` 及其账户状态图标/文案。
- 已登录用户不再渲染该区块，Hero 后直接进入产品分类。
- 未登录用户保留 `loan.loginViewLimit` 登录按钮和原有 `/products/loan` 回跳。
- 删除只为摘要服务的 CSS 和中英文 i18n 键，不改动产品、抵押弹窗、借贷申请或订单流程。

## Acceptance Criteria

- [x] `LoanView` 不包含 `loan-access-pencil__summary`、`loan-access-pencil__icon` 或已登录账户摘要文案。
- [x] 访客登录 CTA 仍存在，且保持 48px 以上触摸目标、主题令牌和登录路由。
- [x] 借贷页顺序、中英文键和高对比断言同步更新。
- [x] 聚焦测试、Mobile 全量测试、type-check、PWA/Tauri 构建与 `git diff --check` 通过。

## Out of Scope

- 不移除借贷页 Hero、产品分类、风险提示或访客登录能力。
- 不修改后端 API 或借贷业务校验。

## Technical Notes

- 主要文件：`mobile/src/views/LoanView.vue`、`mobile/src/i18n/messages/{zh-CN,en}.ts`。
- 相关回归：`mobile/tests/android-ui-secondary-prototype.test.ts`、`mobile/tests/award-ui-secondary-workspaces.test.ts`、`mobile/tests/priority-secondary-page-parity.test.ts`。
