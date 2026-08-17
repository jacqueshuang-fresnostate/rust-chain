# 修复 Turnstile 跨域 postMessage 与重复挂载

## Goal

修复后台 React 管理端和 Vue 手机端登录页的 Cloudflare Turnstile 加载与组件生命周期，避免异步脚本加载、路由离开、主题/语言切换或 React effect 清理之后仍向失效 iframe 发送消息，尽可能消除持续的 `postMessage target origin` 告警，同时保持现有登录与服务端 Siteverify 逻辑不变。

## What I already know

- 后台页面使用 `web/src/auth/LoginPage.tsx`，手机端使用 `mobile/src/views/LoginView.vue`，两者都采用显式渲染。
- 两端现在都将脚本 Promise 保存在组件实例中；组件重建时可能重复注入 API 脚本。
- 两端 `initializeTurnstile()` 在 `await` 之后没有校验当前渲染世代和容器连接状态，旧异步任务可以在组件已清理后继续渲染。
- Ego Browser 已在 `https://hipoex.cllbmz.kdns.fr/login` 和 `https://hippo.cllbmz.kdns.fr/#/login` 复现该告警。
- 线上两端都只加载了一份 Turnstile API 脚本，并能最终生成 token；因此单次 iframe 初始导航期间的短暂告警属于 Cloudflare API 内部时序，业务代码主要需要防止告警因重复挂载而持续累积。
- Cloudflare 官方建议 SPA 使用显式渲染，在 API ready 后创建 widget，并在不再需要时通过 `turnstile.remove(widgetId)` 移除全部关联 DOM。

## Assumptions

- 现有 sitekey/secret 配对和后端 Siteverify 已生效，本任务不轮换凭据。
- `hipoex.cllbmz.kdns.fr` 和 `hippo.cllbmz.kdns.fr` 都已加入当前 widget 的 Hostname Management；线上实测能生成 token 也支持该假设。
- 不为了隐藏控制台告警而捕获或覆盖 `window.postMessage`/`console`。

## Requirements

1. 后台和手机端各自只维护一个模块级 Turnstile API 加载 Promise，组件重建时复用已有脚本。
2. 应用自行注入的非 async/defer 脚本在渲染前等待 `turnstile.ready()`；若部署层已加载 async/defer 脚本且 API 可用，则直接复用 API，避免 Cloudflare 不支持的 `ready()` 组合；加载失败后允许后续重试。
3. 每次初始化使用递增世代号，只有最新任务且容器仍 `isConnected` 时才允许调用 `render()`。
4. 组件卸载、进入二次验证、关闭 Turnstile 或重新渲染时，先使旧世代失效，再调用 `remove(widgetId)`。
5. widget callback 必须校验世代号，旧 widget 不能回写 token 或状态。
6. 保留现有小组件样式、主题、语言、失效/超时回调、登录提交和失败后 reset 行为。
7. 后台切换管理员/代理身份时不重建同一个 widget，避免无意义的 iframe 抖动。

## Acceptance Criteria

- [x] 组件连续挂载/卸载或脚本慢加载时，失效初始化任务不会调用 `render()`。
- [x] 页面上始终最多一个 Turnstile API 脚本和一个当前 widget。
- [x] 路由离开或参数变更时，旧 widget 被调用 `remove()`，旧 callback 不更新 token。
- [x] 后台登录依然传递 `cf_turnstile_token`，管理员/代理切换不增加 render 次数。
- [x] 手机端主题/语言变化仍会生成新的匹配 widget，但不保留旧 iframe。
- [x] 手机端 type-check/test/build:pwa 和后台 typecheck/lint/test/build 通过。
- [x] `git diff --check` 通过。

## Definition of Done

- 单元/组件回归测试覆盖脚本复用、慢加载取消、清理和身份切换。
- 两端最近改动的质量命令全部通过。
- 项目规范记录 Turnstile SPA 生命周期约束。
- `docs/superpowers/PROGRESS.md` 记录本切片。

## Out of Scope

- 不更换 sitekey/secret，不创建新 Cloudflare widget。
- 不修改登录业务规则、后端 Siteverify 策略或 WAF 规则。
- 不通过拦截 `postMessage` 或过滤控制台来伪装修复。
- Cloudflare 第三方脚本在 iframe 导航前自身产生的单次短暂浏览器警告不作为业务失败；本任务保证不因应用端重复挂载导致持续告警。

## Technical Notes

- 官方显式渲染与生命周期：https://developers.cloudflare.com/turnstile/get-started/client-side-rendering/
- 官方 Hostname Management：https://developers.cloudflare.com/turnstile/additional-configuration/hostname-management/
- 现场复现和设计决策见 `research/turnstile-lifecycle.md`。
