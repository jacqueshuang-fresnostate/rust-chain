# 精简秒合约行情面板装饰信息

## Goal

移除手机端秒合约行情面板右下角的装饰性英文文案 `LOCAL / SHORT CYCLE`，并删除顶部 `seconds-round-row` 轮次状态行；保留实时价格、图表、活动订单与下单逻辑不变。

## What I already know

- 文案由 `mobile/src/styles/prototype-base.css` 的 `.seconds-market-board::after` 伪元素生成，不是 Vue 模板或 i18n 文案。
- 生产页面通过后置加载的 `mobile/src/styles/prototype-parity.css` 覆盖原型快照，因此生产修正应放在 parity 层，不改写基础快照。
- 浅色主题只改变该伪元素颜色；将其 `content` 设为 `none` 后，深浅主题都会停止渲染。
- `seconds-round-row` 位于 `SecondsView.vue` 的行情面板顶部，仅展示“当前轮次”和状态摘要；其活动订单数据在后续 `seconds-active-orders` 区域仍会完整展示。

## Requirements

- 秒合约页面不再显示 `LOCAL / SHORT CYCLE`。
- 秒合约页面不再渲染 `seconds-round-row` 及“当前轮次”摘要。
- 不删除或改变秒合约行情面板、实时价格、图表、订单和下单逻辑。
- 删除轮次行后价格行自然上移；不改变面板背景、边框或深浅主题配色。
- 增加源码回归断言，防止装饰文案再次显示。

## Acceptance Criteria

- [x] parity 层明确禁用 `.seconds-market-board::after` 内容。
- [x] 秒合约页面深色和浅色模式均不显示该英文标识。
- [x] `SecondsView` 模板与 scoped CSS 不再包含 `seconds-round-row`。
- [x] 活动订单列表、价格、图表和下单控制保持原有顺序及接口合同。
- [x] 秒合约相关聚焦测试通过。
- [x] Mobile type-check、全量测试、PWA/Tauri 构建和 `git diff --check` 通过。

## Out of Scope

- 不调整秒合约其他英文、i18n 文案或业务接口。
- 不修改 Sites/Pencil 原始快照。

## Technical Notes

- 实现文件：`mobile/src/styles/prototype-parity.css`、`mobile/src/views/SecondsView.vue`。
- 回归测试：`mobile/tests/award-ui-trading-workspaces.test.ts`、`mobile/tests/priority-secondary-page-parity.test.ts`。
- 相关规范：`.trellis/spec/mobile/pwa-and-shell.md`。

## Verification

- 聚焦测试：16/16 通过。
- Mobile type-check：通过。
- Mobile 全量测试：360/360 通过。
- PWA 与 Tauri 构建：均通过，各转换 2071 个模块；PWA 生成 134 条预缓存。
- 构建产物检查：基础伪元素声明与 parity `content: none` 覆盖各出现一次，覆盖位于基础声明之后，`seconds-round-row` 为 0 处。
- `prototype-base.css` 与 `SecondsView` 的完整 `script setup` 区块均与 `HEAD` 一致。
- Trellis task validate 与 `git diff --check`：通过。
