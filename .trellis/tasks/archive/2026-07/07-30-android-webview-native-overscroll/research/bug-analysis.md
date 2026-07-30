## Bug Analysis: Android WebView 边界拖动仍有画面拉伸

### 1. Root Cause Category

- **Category**: D - Test Coverage Gap
- **Specific Cause**: 上一轮把 DOM 根滚动边界和 Android 原生 WebView
  EdgeEffect 当成同一层行为，只断言 CSS 计算样式与手势结束后的 `scrollY`。
  Android 合成器可以在 `scrollY` 始终合法时临时拉伸画面，因此原验收指标无法
  观察用户实际反馈的问题。

### 2. Why Fixes Failed

1. CSS `overscroll-behavior: none`: 只完整解决浏览器/PWA 的滚动链与边界反馈，
   未关闭 Android host WebView 的原生 EdgeEffect。
2. 真机 `scrollY` 边界检查: 只验证手势结束后的逻辑滚动位置，没有采样拖动
   过程中的合成画面，产生错误完成结论。
3. 直接修改生成工程的想法: `src-tauri/gen/` 被忽略，无法在干净检出或重新
   init 后稳定交付。

### 3. Prevention Mechanisms

| Priority | Mechanism | Specific Action | Status |
|----------|-----------|-----------------|--------|
| P0 | Architecture | 在 `onWebViewCreate` 设置 `OVER_SCROLL_NEVER` | DONE |
| P0 | Delivery | 受跟踪 Activity 模板通过 Android runner 同步到生成工程 | DONE |
| P0 | Test Coverage | 锁定原生回调、常量与 init/build 同步时序 | DONE |
| P1 | Verification | 检查编译字节码并在真机拖动过程中验收视觉状态 | IN PROGRESS |
| P1 | Documentation | 区分 CSS scroll boundary 与 Android native EdgeEffect | DONE |

### 4. Systematic Expansion

- **Similar Issues**: iOS `WKWebView` 的 bounce 也不能仅靠 Android 或 CSS
  结论推断；需要独立平台验收后再制定策略。
- **Design Improvement**: 所有原生壳定制必须有受跟踪源文件，生成目录只能是
  构建产物。
- **Process Improvement**: 动画、形变、闪烁类问题必须验收手势或动画进行时
  的画面，不能只检查结束状态和 DOM 数值。

### 5. Knowledge Capture

- [x] 更新 `.trellis/spec/mobile/pwa-and-shell.md`
- [x] 增加 Android 原生源码合同测试
- [x] 记录生成目录同步约束
- [x] 完成 Android 真机进行中拖动验收
