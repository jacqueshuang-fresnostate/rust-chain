# 手机借贷抵押资产弹窗与 Logo

## Goal

将手机端借贷申请中的抵押资产原生下拉框改为与当前 Pencil 移动端体系一致的底部资产选择弹窗，并使用 `/wallet/accounts` 返回的资产 Logo，让用户能清楚识别当前抵押资产及其可用余额。

## Requirements

- 仅影响生产手机端 `LoanView` 的抵押资产选择流程，不改动后端借贷接口、贷款产品选择、抵押数量校验或提交载荷。
- 抵押资产字段使用按钮式选择器，点击后打开底部弹窗，不再使用原生 `<select>`。
- 选择器触发器同时显示当前资产 Logo、币种符号和可用余额；Logo 来源为 `WalletAccount.logoUrl`，继续由 `AssetMark` 提供文字兜底。
- 弹窗列出当前钱包账户资产，每行显示 Logo、币种符号和可用余额，当前资产有明确选中态。
- 点击资产行后更新 `collateralAssetId`、清除当前反馈信息并关闭弹窗；现有 `selectedCollateral`、可用余额校验和 `applyLoan` 参数保持权威。
- 未登录时触发器保持禁用；无钱包账户时弹窗提供明确空态，不伪造资产或 Logo。
- 弹窗使用 `role="dialog"` + `aria-modal="true"`，支持遮罩、关闭按钮、Escape 关闭、Tab 焦点循环、背景滚动锁定和关闭后焦点恢复。
- 所有新增文案同步中英文 i18n；交互控件触摸目标不小于 44px，兼容明暗主题和底部安全区。

## Acceptance Criteria

- [x] 抵押借贷申请中不再出现抵押资产原生 `<select>`。
- [x] 点击抵押资产触发器会打开底部弹窗，触发器与列表行都传入 `WalletAccount.logoUrl` 给 `AssetMark`。
- [x] 弹窗选择资产后贷款提交仍使用对应 `assetId` 和原有抵押数量。
- [x] 弹窗的遮罩/Escape/关闭按钮、焦点循环、滚动锁定和焦点恢复都有源码合同测试。
- [x] 中英文资产选择与空态文案完整。
- [x] 聚焦回归测试、Mobile 全量测试、type-check、PWA/Tauri 构建和 `git diff --check` 通过。

## Definition of Done

- 代码、i18n、回归测试和必要的 Mobile 规范同步更新。
- 只使用现有 Vue/Lucide/AssetMark/弹层样式能力，不引入新依赖。
- 任务验证通过并记录到 `docs/superpowers/PROGRESS.md`。

## Technical Approach

- 复用 `SwapView` 已有的 `pencil-sheet-mask` / `pencil-sheet` 底部弹窗结构，在 `LoanView` 内保持借贷专用状态与样式。
- 用 `collateralPickerOpen` 和 `collateralPickerDialog` 管理弹窗，将订单操作弹窗与抵押资产弹窗合并到一个滚动锁定/焦点恢复观察链，避免两套 watcher 相互覆盖 `body.style.overflow`。
- 列表直接使用已有 `accounts: WalletAccount[]`；字段与列表的 `AssetMark` 都绑定 `logoUrl`。

## Decision (ADR-lite)

**Context**: 原生 select 无法呈现资产 Logo，也与现有移动端资产选择体系不一致。  
**Decision**: 使用本地底部弹窗和 `AssetMark` 实现资产选择，以钱包账户返回的 `logoUrl` 为图片事实源，以币种符号为失败兜底。  
**Consequences**: 资产识别更清晰且可扩展；需要同时维护弹窗的键盘焦点和滚动边界。

## Out of Scope

- 不修改后端资产、钱包或借贷 API。
- 不新增抵押资产搜索、资产分类或估值换算。
- 不重设借贷产品列表、借款订单或订单操作确认弹窗。

## Technical Notes

- 目标文件：`mobile/src/views/LoanView.vue`、`mobile/src/i18n/messages/{zh-CN,en}.ts`、`mobile/tests/trading-lending-views.test.ts`。
- `fetchWalletAccounts()` 已将后端 `logo_url` 适配为 `WalletAccount.logoUrl`，本任务无需调整 API adapter。
- `AssetMark` 已支持 `src` 与图片失败后的币种符号兜底。
- 共享弹窗样式位于 `mobile/src/styles/pencil-selected-pages.css`，借贷页只需增加局部行布局与选中态。
