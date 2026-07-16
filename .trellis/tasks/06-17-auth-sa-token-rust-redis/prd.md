# 迁移认证到 sa-token-rust Redis

## Goal

把后端用户端、管理端、代理端的 access token 签发与校验切换到 `sa-token-rust`，并使用 Redis 持久化登录态；PC 用户端和管理后台继续使用现有 Bearer token 调用方式，避免重写登录页面和业务 API。

## What I Already Know

- 用户明确要求改成 `sa-token-rust`，使用 Redis，用户端和管理端都要改。
- 当前后端使用 `jsonwebtoken` 生成 access token，refresh token 哈希落 MySQL `refresh_tokens`。
- 当前 PC 端与管理端都通过 `Authorization: Bearer <access_token>` 调接口，登录响应字段依赖 `access_token`、`refresh_token`、`token_type`、`scope`。
- 项目当前是 Axum `0.7`，`sa-token-plugin-axum` 0.1.18 默认绑定 Axum `0.8`，直接使用插件会扩大框架升级范围。
- 项目已有 Redis 连接配置 `REDIS_URL`，运行时已有 `AppState.redis` 给行情/闪兑/秒合约等模块使用。

## Assumptions

- 保持现有 HTTP API shape 不变：登录、注册、2FA 登录、refresh 返回字段不改。
- 本任务先覆盖 user/admin/agent 三类 scope，因为代码已有三类 `TokenScope` 与 Extractor。
- Refresh token 不再继续依赖 MySQL `refresh_tokens` 表作为登录态真源；改为 Redis session，并保持密码修改后能撤销用户 refresh token。
- 不在本任务里升级 Axum 到 0.8，也不引入 sa-token Axum middleware。

## Requirements

- 后端新增 sa-token-rust session 管理，access token 由 `SaTokenManager` 生成并存储到 Redis。
- user/admin/agent 使用 `login_type` 隔离，Extractor 必须严格区分 scope。
- refresh token 使用 Redis 保存 actor/scope/user_id/过期时间，并能刷新生成同 scope 的新 access token。
- PC 用户端、管理后台、代理后台继续发送 Bearer token；若后端返回 401，仍清理本地登录态。
- `/ws/private?token=` 必须继续接受新的 sa-token access token。
- 密码修改后需要撤销该用户旧 refresh token，并返回新的 token。
- 保留测试辅助 token 签发能力，让现有路由测试可以按需注入 sa-token 内存/Redis 会话，不被纯 JWT 假 token 绕过。

## Acceptance Criteria

- [ ] `Cargo.toml` 使用 `sa-token-core` 与 Redis 存储能力；main 运行态不再依赖 `jsonwebtoken` 作为业务 access token 校验路径，未注入 auth manager 的轻量测试可保留旧 JWT 兼容分支。
- [ ] `UserAuth`、`AdminAuth`、`AgentAuth` 对 Bearer token 做 Redis session 校验，scope 错误返回 403，缺失/无效 token 返回 401。
- [ ] 登录、注册、2FA 登录、refresh 的响应字段兼容现有 PC 与后台代码。
- [ ] refresh token 过期、scope 不匹配、actor 已停用时拒绝刷新。
- [ ] PC 与 web 管理端有测试覆盖 Bearer header 仍按 token 注入，登录响应字段仍被正确保存。
- [ ] 后端最贴近的 auth/token 单测通过，前端相关 auth/request 测试通过，必要时执行 `cargo check`。

## Definition of Done

- Tests added/updated for auth token issuance/extraction/refresh/revoke.
- `cargo fmt` and closest backend tests pass.
- PC/web auth request tests pass where touched.
- `docs/superpowers/PROGRESS.md` records deliverable slice.
- Auth session contract documented under `.trellis/spec/backend/` if implementation creates a new convention.

## Out of Scope

- 不重做 PC 或后台登录 UI。
- 不升级 Axum 主版本。
- 不实现权限/角色细粒度鉴权，本任务只替换登录态 token/session。
- 不迁移历史 MySQL refresh token 数据；旧 token 失效，用户重新登录即可。

## Research References

- [`research/sa-token-rust-redis.md`](research/sa-token-rust-redis.md) — sa-token-rust 0.1.18 的 Redis/session 能力、Axum 版本约束和本项目集成方案。

## Technical Notes

- 重点文件：`src/modules/auth/mod.rs`、`src/modules/auth/routes.rs`、`src/modules/user/routes.rs`、`src/state.rs`、`src/main.rs`、`pc/src/api/request.ts`、`web/src/api/client.ts`。
- 由于前端已经抽象 Bearer header，预计 PC 和管理端主要是兼容测试/类型约束调整，而不是页面重写。
