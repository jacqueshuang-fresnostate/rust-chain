# PC端鉴权卡片隐藏多余span

## Goal

PC 端登录、注册、忘记密码等鉴权卡片顶部只需要显示平台 Logo 图片，不需要显示 `BrandLogo` 组件渲染的平台名称 `span`。

## What I Already Know

- 用户提供的 selector 指向鉴权页面卡片顶部的 `BrandLogo` 内部 `span`。
- `pc/src/components/common/BrandLogo.vue` 只有在传入 `show-name` 时才渲染平台名称 `span`。
- `Login.vue`、`Register.vue`、`ForgotPassword.vue` 都在卡片顶部传入了 `show-name`。
- Header 也使用 `BrandLogo show-name`，但不在用户提供的鉴权卡片 selector 内，不应受影响。

## Requirements

- 登录页鉴权卡片顶部不显示平台名称 `span`。
- 注册页鉴权卡片顶部不显示平台名称 `span`。
- 忘记密码页鉴权卡片顶部不显示平台名称 `span`。
- 不修改 `BrandLogo` 的默认能力，不影响 Header 等其它可显示平台名称的位置。
- 增加轻量测试防止鉴权页面重新传入 `show-name`。

## Acceptance Criteria

- [x] `pc/src/views/auth/Login.vue` 的卡片顶部 `BrandLogo` 不传入 `show-name`。
- [x] `pc/src/views/auth/Register.vue` 的卡片顶部 `BrandLogo` 不传入 `show-name`。
- [x] `pc/src/views/auth/ForgotPassword.vue` 的卡片顶部 `BrandLogo` 不传入 `show-name`。
- [x] `pc/src/components/common/BrandLogo.vue` 仍保留 `showName` 能力供其它页面使用。
- [x] PC 端相关测试和类型检查通过。

## Definition of Done

- 只修改 PC 鉴权页面 Logo 展示和相关轻量测试。
- 执行最贴近改动的 PC 测试和类型检查。
- 更新 `docs/superpowers/PROGRESS.md`。

## Technical Approach

从三个鉴权页面的顶部 `BrandLogo` 使用处移除 `show-name` 和无效的 `name-class`，保留 Logo 图片尺寸与容器居中。新增 Node 文件源码级测试，确认鉴权页面不再传入 `show-name`，同时 `BrandLogo.vue` 仍支持 `showName` 属性。

## Out of Scope

- 不修改 Header 品牌展示。
- 不重构 `BrandLogo` 组件。
- 不调整鉴权表单布局和其它输入图标 `span`。
