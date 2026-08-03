# 修复深色主题白线残留

## Goal

修复移动端深色主题下页面、卡片、导航和图表区域出现不应有的白色线条/浅色边框，让深色主题使用一致的石墨分隔线，同时保持浅色主题、路由、业务数据和交互不变。

## Requirements

- 定位真机截图中白线的 CSS 来源，区分真实分隔线、焦点环和误用的浅色边框。
- 仅在深色主题覆盖错误的白色线条，保留可读的低对比度分隔线和键盘焦点可见性。
- 覆盖首页、底部导航、Markets、Assets、Profile、Trade/Spot 等共享外壳与主要页面。
- 更新或新增深色主题样式回归断言，确保浅色主题令牌不被破坏。
- 运行移动端测试、类型检查、PWA/Tauri 构建和差异检查。
- 重新生成并安装 Android Debug APK，确认真机深色首页与至少一个二级页无误用白线。

## Out of Scope

- 不修改 API、WebSocket、路由、表单、业务状态或 K 线数据引擎。
- 不改变旧版首页布局和底部导航结构。

## Acceptance Criteria

- [x] 深色主题中错误白线被替换为一致的石墨分隔线。
- [x] 浅色主题和焦点可访问性保持不变。
- [x] 聚焦/全量测试、类型检查、PWA/Tauri 构建和 `git diff --check` 通过。
- [x] 最新 Android Debug APK 安装到已连接手机并完成真机复验。
- [x] `docs/superpowers/PROGRESS.md` 记录修改和验证结果。

## Implementation Notes

- 根因：目标华为 TAS-AL00 的 Android WebView 将 `box-shadow` 中的 `color-mix(..., transparent)` 错误解析为不透明的 `currentColor`，深色主题因此出现纯白轨道。
- 修复：共享外壳、底栏、输入、卡片、资产/我的、订单簿和行情详情阴影改用直接 alpha/石墨令牌；保留浅色主题与焦点环规则，并增加 WebView 兼容性回归断言。
- 验证：聚焦主题测试 8/8、全量测试 227/227、类型检查、PWA/Tauri 构建、Android Debug 构建及 `git diff --check` 均通过；APK SHA-256 为 `5bd254caa7bd5a98a579a7b0195766cf69083d3d3bbd433ebd75b37cc9410673`。已覆盖安装到 Huawei TAS-AL00 并冷启动，真机深色首页及 Orders 二级页复验通过。
