# 手机行情详情图表切换控件左上对齐

## Goal

将手机端行情详情页的 `market-detail__chart-toggle` 从图表右上角移动到左上角，让内联图表与沉浸展开图表保持相同方向，同时不改变展开/收起行为和图表引擎切换器。

## What I already know

- 控件位于 `MarketDetailView.vue` 的 `.market-detail__chart` 内并使用绝对定位。
- 内联状态当前为 `right: 16px; top: 12px`。
- 展开状态当前为 `right: 10px; top: 8px`。
- 图表引擎切换器位于右侧，不会与左上角展开控件重叠。

## Requirements

- 内联图表切换控件定位为左上角，保持现有垂直间距、尺寸、层级和触控行为。
- 沉浸展开状态同样定位到左上角，保留展开状态现有安全间距。
- 不修改按钮 DOM、图标、可访问名称、展开/收起逻辑、焦点恢复或滚动锁。
- 不影响图表引擎切换器、K 线、指标及市场数据区域。

## Acceptance Criteria

- [x] 默认状态使用 `left: 16px; top: 12px`，不再设置 `right`。
- [x] 展开状态使用 `left: 10px; top: 8px`，不再设置 `right`。
- [x] 现有展开/收起可访问性和交互测试保持通过。
- [x] 手机端聚焦测试、type-check、完整测试和 `git diff --check` 通过。

## Out of Scope

- 不调整图表引擎切换器的位置。
- 不修改图表尺寸、时间周期栏、K 线渲染或行情接口。
- 不重构行情详情页其他控件。

## Technical Notes

- 实现：`mobile/src/views/MarketDetailView.vue`。
- 回归测试：`mobile/tests/market-detail-reference-layout.test.ts`。
- 需遵循 `.trellis/spec/mobile/index.md` 的 44px 交互、响应式和质量门要求。

## Definition of Done

- 两种图表状态定位、回归测试、质量检查和进度记录全部完成。
