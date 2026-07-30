# 修复手机端页面边界拉伸

## 背景

Android WebView 在文档滚动到顶部或底部后继续拖动时，会显示系统的弹性拉伸反馈。当前移动端只在 `.app-frame` 禁止横向越界，实际文档滚动根 `html/body` 没有纵向边界策略，因此真机滑动会产生页面被拉扯的感觉。

## 目标

- 禁止根文档在纵向滚动边界触发拉伸或滚动链。
- 保留正常纵向滚动、惯性滚动、黏性 Header、图表手势和输入交互。
- 保留底部弹窗等已有局部滚动容器的滚动行为。
- 同时覆盖浏览器 PWA 与 Tauri Android WebView。

## 非目标

- 不改动路由转场和 GSAP 启动动画。
- 不使用 JavaScript 拦截 `touchmove`。
- 不把 `body` 设为固定高度或全局禁止纵向滚动。
- 不调整页面布局、配色、接口或业务逻辑。

## 验收标准

- `html` 和 `body` 的计算样式均禁止纵向 overscroll。
- 页面内容超过视口时仍能从顶部正常滚动到正文区域。
- 全局样式不新增 `touch-action: none`、`overflow-y: hidden` 或触摸事件拦截。
- 移动端聚焦测试、类型检查、全量测试、PWA/Tauri 构建和 Android Debug APK 构建通过。
- 最新 APK 覆盖安装到已连接的 Android 真机并正常进入前台。
