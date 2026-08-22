# 修复杠杆持仓圆形资产 Logo

## Goal

修复手机端杠杆交易持仓卡片中的后台资产图片被局部样式覆盖为小圆角矩形的问题，使真实 Logo 始终保持共享 `AssetMark` 定义的纯圆形图片效果。

## What I already know

- 持仓卡片通过共享 `AssetMark` 渲染后台 `MarginProduct.logoUrl`，尺寸为 24px。
- `AssetMark` 图片态本身已经具备等宽高、`border-radius: 50%`、`overflow: hidden` 和无高光/无描边/无阴影样式。
- Ego Browser 运行时验证显示，同一资产图片在交易头部为 28x28 圆形。
- 把该节点放入 `.contract-position-identity` 后，`.contract-position-identity span` 会覆盖根节点为 `border-radius: 3px`、`padding: 0 5px`，图片继承后也变成 3px 圆角；这是持仓场景独有的级联冲突。

## Assumptions

- 后台 Logo URL、持仓数据和共享 `AssetMark` 数据回退链保持不变。
- 修复应限制在持仓身份标签选择器，避免给真实图片追加装饰或改变其他页面 Logo。

## Requirements

- 持仓卡片中 24px 后台图片保持 1:1 宽高与完整圆形裁切。
- 多空、全仓/逐仓、杠杆倍数标签继续保留现有小圆角标签样式。
- 图片态继续无高光、无边框、无阴影、无额外内边距。
- 不改变杠杆下单、持仓、风控、行情和路由逻辑。

## Acceptance Criteria

- [x] 持仓 Logo 根节点和图片的运行时 `border-radius` 为 50%，`padding` 为 0，宽高相等。
- [x] 持仓标签仍使用 3px 圆角和原有间距。
- [x] 源码回归测试能阻止宽泛 `.contract-position-identity span` 选择器再次出现。
- [x] Ego Browser 在本地杠杆页复验修复前后级联结果。
- [x] 手机端聚焦测试、类型检查、完整测试及 PWA/Tauri 构建通过。

## Quality Review

- 确认最终 scoped CSS 仅对
  `.contract-position-identity > div > .contract-position-badge` 施加标签材质，
  不会命中同级 `AssetMark` 根 `<span>`。
- 回归测试已加强为核对多空、保证金模式、杠杆倍数三类标签语义，
  并检查 Vue scoped CSS 编译结果；内置旧宽泛选择器夹具证明守卫会真实报错。
- 聚焦测试 20/20、Mobile 全量测试 464/464、类型检查、PWA 构建和
  Tauri 构建均通过。

## Definition of Done

- Tests added/updated for the position-specific CSS cascade.
- Mobile type-check, test suite, PWA build, and Tauri build pass.
- Ego Browser runtime verification records circular geometry and computed styles.
- Mobile UI specification and `docs/superpowers/PROGRESS.md` are updated.

## Out of Scope

- 修改后台资产图片内容或上传流程。
- 调整杠杆持仓业务接口、仓位计算或交易行为。
- 重做持仓卡片其余排版。

## Technical Notes

- Relevant production view: `mobile/src/views/TradeView.vue`.
- Shared image component: `mobile/src/components/AssetMark.vue`.
- Existing regression suite: `mobile/tests/asset-mark-material.test.ts`.
- Runtime diagnosis: `research/css-cascade-diagnosis.md`.
