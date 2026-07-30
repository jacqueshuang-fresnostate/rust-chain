# 修复 Android 首页 Header 安全区溢出

## Goal

修复生产 `mobile/` 客户端首页 RootHeader 在 Android 刘海屏/状态栏安全区较高时，Logo、主题按钮和消息按钮越过 Header 底边的问题，并在已连接的 TAS-AL00 真机上完成构建、安装和运行时边界验证。

## Observed Failure

- 真机 WebView 视口宽度为 360 CSS px，页面横向滚动宽度同为 360px，不存在横向溢出。
- `env(safe-area-inset-top)` 实测为 35px。
- RootHeader 固定高度为 64px，同时使用 35px 顶部 padding。
- Header 的 `scrollHeight` 为 71px；Logo 和 44px Header 控件的底边均为 71px，超出 Header 底边 7px。

## Requirements

- RootHeader 的普通内容轨继续保持 56px，并保留至少 8px 顶部呼吸空间。
- 当 `safe-area-inset-top` 大于 8px 时，RootHeader 总高度必须同步增加，不能压缩或裁切 44px 控件。
- 无安全区或安全区不超过 8px 的浏览器中，RootHeader 总高度继续保持 64px。
- Logo、主题按钮、消息按钮必须完全位于 RootHeader 边界内。
- 不修改二级 Header、路由、业务接口、主题切换和消息中心导航逻辑。
- 不引入新的 UI 依赖或 JavaScript 尺寸监听。

## Acceptance Criteria

- [x] CSS 明确定义 RootHeader 顶部安全区变量和自适应总高度。
- [x] 0px 与 8px 安全区下 Header 总高度为 64px。
- [x] 35px 安全区下 Header 总高度足以完整容纳 44px 控件。
- [x] 真机运行时 `header.scrollHeight <= header.clientHeight`。
- [x] 真机 Logo 与两个 Header 控件的 `bottom <= header.bottom`。
- [x] 360px 真机视口中 `document.scrollWidth === innerWidth`。
- [x] 聚焦测试、移动端全量测试、类型检查和 Android Debug APK 构建通过。

## Out Of Scope

- 不调整首页主体、底部导航、二级页面 Header 或设计色板。
- 不修改 Android 原生状态栏/窗口配置。
- 不提交或推送当前工作区中的既有未提交改动。

## Definition Of Done

- 实现和回归测试完成。
- Android Debug APK 安装到已连接设备并启动。
- 通过 WebView 调试协议记录修复后的真实边界。
- 更新移动端规范和 `docs/superpowers/PROGRESS.md`。
