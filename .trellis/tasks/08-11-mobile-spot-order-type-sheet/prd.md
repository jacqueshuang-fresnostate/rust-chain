# 手机现货订单类型使用选择弹窗

## Goal

将手机端现货交易页的 `spot-type-field` 从“点击后直接在限价单/市价单之间切换”改成明确的订单类型选择入口：点击字段打开底部弹窗，用户在弹窗内选择限价单或市价单后再更新表单。

## What I Already Know

- 现有实现位于 `mobile/src/views/TradeView.vue`，`toggleSpotOrderType()` 会在一次点击时直接切换 `orderType`，容易误触且不符合用户指定交互。
- 页面已有 `orderType: 'limit' | 'market'` 状态，现有价格只读、有效价格、确认和下单请求逻辑都已按该状态联动，无需修改后端接口。
- 项目已有 `useModalDialog`，可复用背景滚动锁定、Escape 关闭、Tab 焦点环和关闭后的焦点恢复。
- 项目已有 Pencil 底部弹层视觉语言、Lucide 图标、明暗主题变量和安全区处理。

## Requirements

- 点击 `spot-type-field` 只打开订单类型底部弹窗，不直接修改当前订单类型。
- 弹窗提供“限价单”和“市价单”两个独立选项，并明确显示当前选中项。
- 点击选项后更新 `orderType` 并关闭弹窗；限价/市价原有价格字段、有效价格和下单参数联动保持不变。
- 点击遮罩、关闭按钮或按 Escape 关闭弹窗时，不修改当前订单类型。
- 弹窗锁定背景滚动、约束 Tab 焦点，关闭后焦点返回 `spot-type-field`。
- 弹窗提供正确的 `role="dialog"`、`aria-modal`、标题关联和选项选择状态；所有交互目标不小于 44px。
- 所有可见文案使用中英文 i18n；图标统一使用 Lucide，不使用表情符号。
- 明暗主题、底部安全区和 320px–448px 手机宽度下不产生横向溢出。

## Acceptance Criteria

- [x] 点击 `spot-type-field` 后订单类型保持不变，并显示选择弹窗。
- [x] 点击“限价单”或“市价单”后正确更新状态、关闭弹窗并保持现有表单/下单联动。
- [x] 遮罩、关闭按钮和 Escape 只关闭弹窗，不改变选择。
- [x] 当前选项具有可见选中态和可访问的 `aria-pressed` 状态。
- [x] 弹窗具备焦点圈、背景滚动锁和触发器焦点恢复。
- [x] 新增文案在 `zh-CN` 与 `en` 中保持键一致。
- [x] Mobile 聚焦测试、全量测试、type-check 与 PWA build 通过。

## Definition of Done

- 现货订单类型选择弹窗、样式、i18n 和回归测试完成。
- 最贴近改动的 Mobile 质量门通过。
- 移动端可执行规范与 `docs/superpowers/PROGRESS.md` 更新。

## Technical Approach

在 `TradeView` 增加独立的弹窗开关和引用，复用 `useModalDialog` 管理滚动与焦点；触发器只负责打开弹窗，两个选项通过显式 `selectSpotOrderType(type)` 提交选择并关闭。弹层采用项目已有底部 Sheet 结构，但使用页面专属类控制紧凑高度、选中态和安全区，避免影响其他弹窗。

## Decision (ADR-lite)

**Decision**: 使用“单次选择即生效并关闭”的底部弹窗，而不是继续循环切换，也不引入额外确认按钮。

**Consequences**: 用户可以在修改前看见所有可选订单类型和当前状态；交互步骤清晰；原有订单计算和后端请求合同完全保留。

## Out of Scope

- 不修改现货下单 API、订单精度、余额计算或后端订单类型枚举。
- 不调整合约交易页固定市价单逻辑。
- 不新增止盈止损或其他订单类型。
- 不重构现有下单确认弹窗。

## Technical Notes

- 主要实现：`mobile/src/views/TradeView.vue`。
- 文案：`mobile/src/i18n/messages/zh-CN.ts`、`mobile/src/i18n/messages/en.ts`。
- 回归测试：优先扩展 `mobile/tests/spot-trading-ui-optimization.test.ts`，覆盖触发、选择、关闭、焦点和 i18n 合同。
- 复用：`mobile/src/core/modalDialog.ts`。
