# 恢复旧版移动端视觉体系并完成手机真机预览

## Goal

将 HIPPO Mobile 底部导航、共享壳层及主要页面恢复为与旧版首页一致的清透网格、薄荷绿色主动作、细边框卡片和旧版导航体系，再生成 Android Debug 应用并安装到已连接的实体手机，让用户直接查看统一后的效果。

## What I already know

- 项目通过 `mobile/scripts/run-android-tauri.mjs` 构建 Android Tauri 应用。
- Android 包名为 `com.hippo.exchange.mobile`，入口为 `.MainActivity`。
- 最新已记录的 aarch64 Debug APK 位于 `mobile/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`。
- 当前工作区包含尚未提交的手机端视觉与双 K 线引擎改动，安装时不得回退或混入无关修改。
- 用户已确认不接受刚安装的新版首页，要求恢复此前首页样式；仓库中的 `mobile/src/views/HomeView.vue` Git 基线与 `research/screenshots/award-audit/home-light.png` 共同记录了目标旧版首页。
- 用户进一步要求底部导航恢复，并让其他主要页面沿用首页的旧版风格；`research/screenshots/award-audit/{markets,assets,profile,messages,loan,security,trade}-light.png` 是各页面旧版视觉参考，`after-*.png` 是需要收敛的当前样式对照。

## Assumptions

- 用户所说的已插入手机是已开启 USB 调试并可授权 ADB 的 Android 实体设备。
- 若现有 APK 晚于全部相关手机端源码，可直接覆盖安装；否则先重新构建。
- 保留各页面当前真实 API、WebSocket、路由、表单和确认行为，仅调整共享壳层、底部导航和视觉层级；不得回退最近的本地 K 线引擎与行情数据修复。

## Requirements

- 识别实体设备、型号、序列号与 ADB 授权状态。
- 将首页恢复为旧版顺序：顶部搜索、资产概览与真实账户估值、买币/充币、八项产品入口、行情简报、行情列表与底部权益入口。
- 恢复旧版资产折线、周期控制和首页公告卡视觉；不得回退其他页面或通用业务行为。
- 更新首页相关测试，使其验证旧版首页合同，同时保留行情页与全局外壳现有合同。
- 恢复七项底部导航的旧版网格/浮起绿色 Seconds 控件/当前项样式，保持路由和可访问性属性不变。
- 统一 Markets、Assets、Profile、Message、Loan、Security、Trade、Seconds 的页面背景、Hero、卡片、列表、按钮和状态面板到首页的旧版视觉语言；页面仍显示真实数据和现有状态分支。
- 为共享视觉合同和每类主要页面补充或更新可执行回归断言。
- 确认安装产物与当前源码的新旧关系，必要时重新生成 aarch64 Debug APK。
- 使用覆盖安装保留应用数据，随后强制停止并冷启动应用。
- 验证 `MainActivity` 位于前台且可见，并记录 APK 校验信息与安装时间。
- 不修改任何手机端业务代码，也不纳入或回退现有工作区改动。

## Acceptance Criteria

- [x] ADB 中至少有一台状态为 `device` 的实体 Android 手机。
- [x] 首页模板、首页专属覆盖样式和测试恢复为 Git 基线中的旧版首页合同。
- [x] 底部导航恢复为旧版绿色浮起 Seconds、七栏网格和当前项样式，路由行为保持不变。
- [x] 主要页面在明暗主题与窄屏下共享首页的旧版视觉合同，真实数据和交互行为不回退。
- [x] 聚焦/全量测试、类型检查与 Android Debug 构建通过。
- [x] 最新源码对应的 Debug APK 安装返回 `Success`。
- [x] `com.hippo.exchange.mobile/.MainActivity` 成功启动并处于前台恢复态，已抽查首页、行情、资产、交易和我的页面。
- [x] 更新 `docs/superpowers/PROGRESS.md`，记录设备、APK 与验证结果。

## Definition of Done

- 执行与本次安装最贴近的设备、包和前台状态验证。
- 保留现有未提交工作，不触碰无关文件。
- 明确记录任何由设备授权或本机工具链导致的阻断。

## Out of Scope

- 修改页面业务功能、接口、WebSocket、路由、表单和数据适配逻辑。
- 清除应用数据、卸载应用或修改手机系统设置。
- 提交或推送当前工作区中的既有改动。

## Technical Notes

- Android 构建：`npm --prefix mobile run tauri:android:build -- --debug --target aarch64 --apk`。
- 安装：`adb -s SERIAL install -r APK`。
- 启动：`adb -s SERIAL shell am force-stop com.hippo.exchange.mobile` 后使用 `am start -W`。
- 规范参考：`.trellis/spec/mobile/index.md`、`.trellis/spec/mobile/pwa-and-shell.md`。
- 目标截图：`.trellis/tasks/07-31-mobile-market-detail-reference-layout/research/screenshots/award-audit/home-light.png`。
- 主要页面旧版参考：`.trellis/tasks/07-31-mobile-market-detail-reference-layout/research/screenshots/award-audit/{markets,assets,profile,messages,loan,security,trade}-light.png`。
