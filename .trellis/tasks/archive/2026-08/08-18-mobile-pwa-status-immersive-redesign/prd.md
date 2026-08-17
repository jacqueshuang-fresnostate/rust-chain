# 沉浸式重构手机端 PWA 状态浮层

## Goal

将当前贴边、扁平的 `PwaStatus` 状态条升级为与 HIPPO 手机端精品视觉一致的沉浸式系统浮岛，在不改变安装、更新、离线、离线就绪和注册失败业务行为的前提下，提升层次、材质、状态辨识、动效和窄屏可用性。

## What I already know

- `PwaStatus` 只在 `mobile/src/App.vue` 挂载一次，业务状态来自 `mobile/src/pwa/index.ts`。
- 安装、更新、离线就绪和注册错误只在安全路由提示；离线状态允许全局出现。
- 当前组件位于 Header 下方，使用全宽直角状态带、左侧 3px 色条且无阴影，用户认为不够沉浸。
- 项目使用 Vue 3、现有双主题令牌和 Lucide 图标；不新增视觉依赖，不使用表情符号。
- PWA 浮层不能伪装离线能力，也不能改变更新由用户确认后才重载的合同。

## Requirements

1. 采用“顶部悬浮系统浮岛”而非全屏阻塞弹窗：保留 Header 下方安全位置，但与左右边缘、Header 明确脱离。
2. 使用 Ethereal Glass + Z 轴状态叠层：双层外壳、内侧折射高光、克制的状态环境光、细微网格/噪点和有质量感的阴影。
3. 每种状态继续使用真实 Lucide 图标和语义色：安装/更新为 accent、离线/错误为 negative、离线就绪为 positive。
4. 标题、说明、状态标识和操作形成清晰层级；安装、更新、重试、稍后和关闭操作保持原函数及原有 i18n 文案。
5. 根层保持 `pointer-events: none`，只有浮岛及按钮接收交互；不锁定页面滚动、不拦截底层页面，也不改变安全路由白名单。
6. 按钮和关闭控件至少 44×44；焦点环完整；忙碌态清晰；320px 下操作区可稳定折行且不产生横向溢出。
7. 进入/离开和内容层使用 transform/opacity 的自定义弹性曲线；`prefers-reduced-motion` 下取消位移、模糊和装饰动画。
8. 明暗主题均使用现有语义令牌和 `color-mix`，不重新引入退役的 `#0b1811` 色族，不添加外部图片或运行时资源。
9. 保留离线状态可与一个主要 PWA 状态同时出现，以及现有 update > install > offline-ready > error 的显示优先级。

## Acceptance Criteria

- [x] `PwaStatus` 仍只挂载一次，安全路由、状态判断和所有 PWA 方法调用保持不变。
- [x] 组件在 320、390、448px 宽度下与 Header/画布边界分离，文档宽度不超出视口。
- [x] 浮岛具备可执行测试覆盖的双层玻璃外壳、状态环境光、内侧高光和自定义入场动效。
- [x] 所有可见按钮至少 44px，键盘焦点完整，禁用态不会误触。
- [x] 明暗主题的标题、说明、按钮、状态图标均保持可读，不依赖颜色单独表达状态。
- [x] reduced-motion 禁止非必要位移、模糊和循环装饰动画。
- [x] 手机端聚焦测试、全量测试、type-check、PWA build、Tauri build 与 `git diff --check` 通过。
- [x] Ego Browser 在 320×720、390×844、448×900 的明暗主题检查无横向溢出并完成视觉核验。

## Definition of Done

- 组件结构和样式完成，原业务逻辑无回归。
- 源码合同测试更新，覆盖沉浸式视觉、状态语义和窄屏约束。
- 浏览器与构建验证完成。
- Trellis 规范和 `docs/superpowers/PROGRESS.md` 已同步。

## Technical Approach

- 保留组件现有 script 状态与事件函数，重点重组 template 和 scoped CSS。
- 每个状态卡采用外层 `article/section` + 内层 panel 的 Double-Bezel 结构；根浮层使用非交互环境光层。
- 通过 Vue `Transition` 管理浮层整体进入/离开，卡片内部用固定延迟实现轻量层次，不引入 GSAP 或新包。
- 在现有 source-contract 测试基础上新增 `pwa-status-immersive.test.ts`，并修改仍断言旧直角状态带的测试。

## Decision (ADR-lite)

**Context**：全屏 Modal 更“沉浸”，但会遮挡页面、需要焦点陷阱和滚动锁，并可能干扰用户正在查看的行情或页面状态。

**Decision**：采用顶部悬浮、非模态的沉浸式系统浮岛；通过玻璃材质、环境光与 Z 轴动效提升视觉，而不是扩大交互阻塞范围。

**Consequences**：视觉更强但仍保持 PWA 提示的辅助属性；页面继续可操作，现有路由安全边界和无障碍 live region 不变。

## Out of Scope

- 不修改 Service Worker、manifest、缓存策略或更新协议。
- 不新增 PWA 状态、不改变 iOS 安装指引流程。
- 不修改其他弹窗、Header、底部导航或页面业务样式。
- 不引入新图标库、动画库、图片或远程视觉资源。

## Technical Notes

- 主要文件：`mobile/src/components/PwaStatus.vue`、`mobile/tests/pwa.test.ts`、`mobile/tests/ui-prototype-alignment-foundation.test.ts`。
- 现有 PWA 合同：`.trellis/spec/mobile/pwa-and-shell.md`。
- 视觉审计与设计选择：`research/current-surface-audit.md`。
