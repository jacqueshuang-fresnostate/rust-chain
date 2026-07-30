# 修复 Android WebView 顶底弹性拉伸

## 背景

上一轮仅通过 CSS `overscroll-behavior: none` 和最终 `scrollY` 验证滚动边界，
没有覆盖 Android 12 WebView 在手指越过顶部或底部时产生的原生 EdgeEffect
画面拉伸。用户真机体验确认该弹性形变仍然存在。

## 目标

- 在 Tauri Android WebView 创建时关闭原生 overscroll EdgeEffect。
- 保留正常纵向滚动、惯性滚动、黏性 Header、图表手势和输入交互。
- 保留浏览器/PWA 的 CSS 根滚动边界策略。
- 让原生 Activity 定制通过受 Git 跟踪的源码和构建脚本稳定进入每次 Android
  构建，不能只修改被忽略的 `src-tauri/gen/android` 生成目录。
- 构建并覆盖安装到当前连接的 Android 手机，验证顶部和底部继续拖动时不再
  出现页面拉伸。

## 非目标

- 不拦截 JavaScript `touchmove`。
- 不关闭页面正常滚动或惯性滚动。
- 不修改页面布局、接口、路由、启动动画或业务逻辑。
- 不编辑 Tauri 自动生成的 `WryActivity`、`RustWebView` 或
  `generated/TauriActivity.kt`。

## 技术方案

- 在受跟踪的 Android `MainActivity` 模板中覆写
  `onWebViewCreate(webView: WebView)`。
- 调用父类回调后设置
  `webView.overScrollMode = View.OVER_SCROLL_NEVER`。
- Android runner 在 `build`/`dev` 前同步模板到生成工程；`init` 成功后同步，
  使重新初始化生成目录后仍能恢复定制。
- 增加源码合同测试，锁定原生回调、常量和同步时序。

## 验收标准

- Android WebView 创建后 `overScrollMode` 为 `OVER_SCROLL_NEVER`。
- `MainActivity` 继续启用 edge-to-edge，且不修改 Tauri 生成基类。
- Android runner 对 build/dev 执行构建前同步，对 init 执行成功后同步。
- 聚焦测试、类型检查、全量测试、Tauri 构建和 Android Debug APK 构建通过。
- 最新 APK 覆盖安装到当前设备并正常冷启动。
- 真机正常上下滚动有效；到达顶部或底部后继续拖动，不再出现原生页面拉伸。
