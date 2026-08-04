# 优化手机端 Turnstile 验证组件

## Goal

优化手机端登录页 Cloudflare Turnstile 的布局、材质、响应式宽度与状态反馈，使验证区域融入现有 Pencil 登录页，并在 320–448px、明暗主题和移动 WebView 中保持清晰、稳定、无横向溢出。

## What I already know

- 手机端使用 Vue 3、Vite 与 Tauri，登录页为 `mobile/src/views/LoginView.vue`。
- Turnstile 使用显式渲染，当前容器只有 `min-height: 82px` 和 `width: 100%`，没有视觉层次、主题参数或柔性尺寸参数。
- Cloudflare 官方显式渲染支持 `size: flexible`、`theme: auto|light|dark`、`appearance` 等参数；`flexible` 适合移动宽度。
- 登录页现有视觉系统为冷白/石墨、薄边框、薄荷状态色和紧凑 Pencil 表单。

## Requirements

- 使用 Cloudflare 官方 `size: flexible`，避免固定宽度在 320px 页面溢出。
- Widget 主题必须跟随应用明暗主题，而不是只跟随系统主题。
- 验证区域只保留居中的 Cloudflare 原生组件，不增加装饰卡片或重复品牌；脚本加载前可显示轻量占位，后续状态由原生组件和无障碍播报承担。
- 第三方 iframe 只由 Cloudflare 渲染；外层样式不得裁切挑战内容或阻断触控。
- 修复现有 widget id 为 `0` 时无法 reset/remove，以及 reset 后错误丢弃现存 widget id 的生命周期问题。
- 保持登录 API、token 回调、两步验证跳转和错误处理不变。

## Acceptance Criteria

- [ ] `turnstile.render` 收到 `size: flexible`、当前主题和明确语言配置。
- [ ] 320、360、390、448px 下验证区域无水平溢出，iframe/容器可完整点击。
- [ ] 浅色、深色主题下原生组件均居中，无重复外层卡片。
- [ ] 等待、成功、失败/过期状态通过 Cloudflare 原生表面和 `aria-live` 保持真实语义。
- [ ] 聚焦测试、类型检查、移动端全量测试、PWA/Tauri 构建通过。

## Definition of Done

- Tests cover render options, lifecycle guards, status markup, and responsive style contracts.
- Browser verification covers 320px and 390px in both themes.
- Progress and mobile shell spec are updated.

## Out of Scope

- 修改 Cloudflare Dashboard Widget 模式或域名白名单。
- 改动后台/PC Turnstile 样式。
- 更改服务端 Siteverify 逻辑。

## Technical Notes

- Main implementation: `mobile/src/views/LoginView.vue`.
- Existing auth tests: `mobile/tests/access-identity-settings-views.test.ts`, `mobile/tests/pencil-selected-unmapped-pages.test.ts`.
- Relevant specs: `.trellis/spec/mobile/pwa-and-shell.md`, `.trellis/spec/mobile/backend-integration.md`.

## Research References

- [`research/cloudflare-widget-options.md`](research/cloudflare-widget-options.md) — official responsive size, theme, appearance, and SPA explicit-rendering options.
