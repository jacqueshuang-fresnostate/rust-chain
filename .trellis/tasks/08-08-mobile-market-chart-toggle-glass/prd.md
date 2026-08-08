# 手机行情图表切换按钮正方形毛玻璃优化

## Goal

在保持行情详情图表切换按钮左上角定位和全部展开交互不变的前提下，将控件升级为具有清晰层次、双主题适配和可访问交互反馈的正方形毛玻璃按钮。

## What I already know

- 控件位于 `MarketDetailView.vue` 的 `.market-detail__chart` 内，普通和沉浸状态均已固定在左上角。
- 当前控件为 32×32px、8px 圆角和不透明混色背景，毛玻璃质感不明显。
- 图表引擎切换器位于右侧，左侧控件放大到规范触控尺寸后仍不冲突。
- 手机端规范要求图标按钮至少 44×44px、SVG 双轴居中并提供完整键盘聚焦反馈。

## Assumptions

- “正方形”解释为等宽等高、明显区别于圆形和胶囊的圆角方形；采用 44×44px 与 12px 圆角。
- 毛玻璃采用半透明渐变、背景模糊和饱和度、细边框、内高光与克制投影组合，并通过现有语义令牌自动适配明暗主题。

## Requirements

- 控件视觉尺寸和触控尺寸统一为 44×44px，保持 Lucide 图标居中。
- 使用半透明渐变、`backdrop-filter` 与 `-webkit-backdrop-filter` 形成真实毛玻璃材质。
- 使用细边框、内侧高光和柔和投影建立玻璃边缘与悬浮层次，禁止使用退役深绿色边框族。
- 提供按压反馈、完整 `:focus-visible` 外环和 reduced-motion 兼容。
- 保留普通 `left: 16px; top: 12px`、展开 `left: 10px; top: 8px` 定位。
- 不修改按钮 DOM、图标、aria、展开/收起、焦点恢复、滚动锁、图表引擎切换器或行情数据链路。

## Acceptance Criteria

- [x] 按钮为 44×44px、12px 圆角的正方形，SVG 中心偏差不超过 0.5px。
- [x] 样式包含半透明渐变、标准与 WebKit 背景模糊、边框、内高光和外投影。
- [x] 按压与键盘聚焦状态清晰，低动态模式不会执行位移或过渡。
- [x] 普通与展开状态继续保持既定左上角偏移，右侧图表引擎切换器不变。
- [x] 320px、390px、448px 视口无横向溢出，明暗主题均可辨识。
- [x] 聚焦测试、mobile type-check、全量测试、PWA/Tauri 构建和 `git diff --check` 通过。

## Out of Scope

- 不重做图表引擎切换器或行情详情页其他按钮。
- 不修改图表尺寸、K 线渲染、行情接口和展开状态布局结构。
- 不引入新的 UI 或动效依赖。

## Technical Notes

- 实现文件：`mobile/src/views/MarketDetailView.vue`。
- 回归测试：`mobile/tests/market-detail-reference-layout.test.ts`。
- 质量合同：`.trellis/spec/mobile/index.md` 与 `.trellis/spec/mobile/pwa-and-shell.md`。

## Definition of Done

- 视觉材质、交互状态、双主题、响应式、回归测试、构建验证和进度记录全部完成。
