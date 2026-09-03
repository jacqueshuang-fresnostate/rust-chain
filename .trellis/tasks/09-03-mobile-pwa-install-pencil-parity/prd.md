# 手机端 PWA 安装提示对齐 Pencil 选稿

## Goal

将手机端现有的 PWA 安装状态浮岛重构为 Pencil 中 `NROQD`（浅色）与 `Tcgl6`（深色）画板定义的安装弹窗，并保持 Android/Chromium 安装、iOS 手动添加、频控、更新、离线和失败状态原有业务能力。

## What I already know

- 用户明确要求 PWA 安装提示弹窗按 Pencil 设计 1:1 实现。
- Pencil 安装弹窗是底部模态层：全画布遮罩、390px 基准下 540px 高、顶部 26px 圆角、20px 水平内边距。
- 安装态包含拖拽条、64px 应用图标、标题与副标题、36px 关闭按钮、说明、三项优势、iPhone 提示、54px 主按钮和 38px“稍后提醒”。
- 浅色/深色画板分别为 `NROQD` / `Tcgl6`，安装弹窗节点分别为 `FwXCx` / `V04kP`。
- 当前 `PwaStatus.vue` 同时承担安装、更新、离线、离线就绪和注册失败状态；只有安装提示需要改成 Pencil 底部弹窗，其余状态继续保持非阻塞系统状态卡，避免扩大范围。
- 现有 `promptPwaInstall()`、`dismissPwaInstall()`、频控及安全路由逻辑继续作为真实业务来源。

## Assumptions (resolved from design and repository)

- 安装弹窗使用现有 HIPPO 横向品牌图，不新增远程图片或第二套图标库。
- 弹窗采用 Lucide `Zap`、`Maximize`、`BellRing`、`Info`、`Download`、`X`。
- Android/Chromium 主按钮触发原生 `beforeinstallprompt`；iOS 不存在原生安装事件时，主按钮将把焦点移到可读的手动安装提示，避免无响应。
- 弹窗在安全路由、安装资格满足且更新提示未占用更高优先级时出现；离线时不主动发起安装。
- 390px 作为像素基准，320–448px 自适应且不产生横向溢出；短屏允许弹窗内部滚动。

## Requirements

1. 复刻 Pencil 安装态的结构、尺寸、间距、颜色、圆角、字体层级与明暗主题。
2. 使用全画布遮罩和底部弹窗；支持遮罩、关闭按钮、“稍后提醒”和 Escape 关闭。
3. 弹窗打开时锁定背景滚动、捕获焦点并在关闭后恢复焦点。
4. 所有图标统一使用 Lucide；主交互满足至少 44px 命中区和可见键盘焦点。
5. 原生安装进行中显示忙碌态并阻止重复点击；安装失败仍由既有错误状态反馈。
6. iOS 手动添加提示始终保留，iOS CTA 不产生无反馈操作。
7. 更新、离线、离线就绪和注册失败状态的既有业务顺序及操作保持不变。
8. 现有 PWA/Tauri 构建隔离、Service Worker 策略和金融请求不缓存合同保持不变。
9. 中文和英文文案资源保持完整，不在模板中硬编码可见业务文案。
10. 支持 `prefers-reduced-motion`，只使用 opacity/transform 实现出入场。

## Acceptance Criteria

- [x] 390px 基准下安装弹窗宽度等于画布、可见高度 540px、顶部圆角 26px、内容关键坐标与 Pencil 相符。
- [x] 浅色使用 `#FFFFFF` 弹窗、`#07110C80` 遮罩及 Pencil 指定文字/面板色；深色使用 `#101A15` 弹窗及对应深色节点色。
- [x] 64px 品牌图标、82px 标题区、44px 说明、146px 优势区、42px iPhone 提示、54px 安装按钮、38px 稍后按钮均有自动化合同。
- [x] Android/Chromium 安装按钮继续调用 `promptPwaInstall()`；iOS CTA 聚焦手动安装提示。
- [x] 关闭、遮罩、稍后、Escape、Tab 循环、背景滚动锁定及焦点恢复可用。
- [x] 320px、390px、448px 不出现横向页面溢出；短屏内容可完整访问。
- [x] 明暗主题无需重载即可正确切换；无 Emoji、远程图片和内联 SVG。
- [x] PWA 聚焦测试、Mobile 类型检查、测试类型检查、源码治理及 `git diff --check` 通过。
- [x] 使用浏览器在 390px 明暗主题下完成视觉核验。

## Definition of Done

- Tests added/updated for Pencil geometry, accessibility and existing PWA behavior.
- Mobile type checks and focused tests pass.
- PWA/spec/progress documentation reflects the new modal contract.
- Existing unrelated dirty changes remain untouched.

## Out of Scope

- 不修改 Service Worker、manifest、安装频控算法或允许路由名单。
- 不重做更新、离线、离线就绪和错误状态的视觉样式。
- 不修改 Admin、PC 或后端业务。
- 不编辑 Pencil 设计文件。

## Technical Notes

- Primary component: `mobile/src/components/PwaStatus.vue`.
- Shared modal lifecycle: `mobile/src/core/modalDialog.ts`.
- Locale resources: `mobile/src/i18n/messages/{zh-CN,en}.ts`.
- Existing PWA behavior: `mobile/src/pwa/index.ts`.
- Pencil truth and node measurements: `research/pencil-pwa-install-dialog.md`.

## Implementation Handoff

- `PwaStatus.vue` now renders only the eligible install branch as the
  `NROQD/FwXCx` and `Tcgl6/V04kP` body-Teleported bottom modal; update,
  offline, offline-ready, and registration/install-error states retain the
  existing status-island structure and priority.
- The modal reuses `useModalDialog`, the existing HIPPO landscape asset, and
  Lucide icons. Chromium keeps the native install call; the iOS CTA focuses
  the readable manual-install hint.
- Review fixed the dark `AgFcl` overlay, the global 44px button-minimum cascade
  on the 38px later face, title tracking, both Pencil shadows, 19px action
  icons, and install-failure visibility ahead of stale ready/install state.
- Focused PWA/shell regressions pass 43/43. Mobile production/test type checks,
  source/test governance, the PWA production build, task-context validation,
  and `git diff --check` pass. Runtime verification at 390px confirmed the
  exact light/dark computed overlays, 390×540 sheet geometry, 350×38 later
  button, zero title tracking, and zero horizontal overflow.
