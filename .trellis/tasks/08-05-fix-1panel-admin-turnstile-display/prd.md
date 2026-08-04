# 修复 1Panel 后台登录 Turnstile 不显示

## Goal

修复集成镜像部署到 1Panel 后，后台登录页在已配置 Cloudflare Turnstile 的情况下仍不展示验证组件的问题，确保运行时配置、公开登录配置接口和后台 SPA 渲染状态保持一致。

## What I already know

- 1Panel Compose 已向 API 容器透传 `CF_TURNSTILE_SECRET`、`CF_TURNSTILE_SECRET_KEY`、`CF_TURNSTILE_SITE_KEY`、`CF_TURNSTILE_SITEVERIFY_URL` 和 `CF_TURNSTILE_ENFORCE_TOKEN`。
- 后台 SPA 与 Rust API 位于同一镜像，由 Nginx 在 8080 端口统一提供。
- Vite 环境变量是镜像构建期配置，1Panel 运行时变量需要通过 `/admin/api/v1/auth/login/config` 下发给已构建的后台 SPA。
- 当前服务端只有在 Secret、Site Key 与 `CF_TURNSTILE_ENFORCE_TOKEN=true` 同时满足时才返回 `cf_turnstile_enabled=true`。
- 当前后台登录页仅在同时获得启用状态和 Site Key 时挂载 Turnstile 容器。

## Assumptions

- 用户希望登录时显示并校验 Turnstile，而不仅是依赖 Cloudflare 页面级挑战或 `cf_clearance`。
- 1Panel 更新环境变量后会重新创建 API 容器，使 Rust 进程读取到最新环境。

## Requirements

- 明确并修复 1Panel 运行时环境变量到后台登录页的完整配置链路。
- 当后端要求 `cf_turnstile_token` 时，登录配置接口必须同步返回启用状态与可用 Site Key。
- 后台登录页必须在配置到达后可靠渲染一次 Turnstile，并处理脚本加载失败、配置刷新和登录失败后的重置。
- 1Panel 示例环境变量必须提供能实际显示组件的明确案例与中文说明。
- 不改变管理员、代理登录 API 路径、会话范围和两步验证语义。

## Acceptance Criteria

- [x] `GET /admin/api/v1/auth/login/config` 在 Secret、Site Key、强制 token 开关配置正确时返回 `cf_turnstile_enabled=true` 和非空 `cf_turnstile_site_key`。
- [x] 后台登录页收到上述响应后挂载 `.admin-login-turnstile-widget` 并调用 `turnstile.render`。
- [x] 配置缺失时页面不进入“后端要求 token、前端无组件”的矛盾状态。
- [x] Docker/1Panel 配置案例明确 `CF_TURNSTILE_ENFORCE_TOKEN=true` 才会每次登录展示并校验组件。
- [x] Rust 与后台 Web 的聚焦测试、类型检查、构建和 Compose 配置校验通过。

## Definition of Done

- Tests added/updated for the runtime configuration and admin login rendering path.
- Rust checks and admin lint/typecheck/test/build pass.
- `docker compose config` validates the 1Panel example with sample environment values.
- Progress log updated with root cause and verification.

## Out of Scope

- Cloudflare Dashboard 中 Widget 域名白名单的自动修改。
- PC 与手机端视觉重构。
- 修改 Cloudflare WAF/Managed Challenge 规则。

## Technical Notes

- Relevant backend: `src/modules/auth/routes.rs`.
- Relevant admin client: `web/src/api/adminAuth.ts`, `web/src/auth/LoginPage.tsx`.
- Relevant deployment files: `docker-compose.1panel.yml`, `docker-compose.1panel.example.yml`, `docker-compose.1panel.env.example`, `Dockerfile`, `docker/nginx.conf`.
- Applicable specs: `.trellis/spec/admin/ui-system.md`, `.trellis/spec/backend/user-authentication.md`, `.trellis/spec/backend/container-delivery.md`, `.trellis/spec/guides/cross-layer-thinking-guide.md`.
