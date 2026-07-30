# 初始化默认管理员账号

## Goal

让数据库初始化流程在 SQLx migrations 成功后创建首个后台管理员，
从而使全新部署可以直接获得明确的后台登录账号，同时不覆盖任何已有管理员或密码。

## What I already know

- `exchange-migrate` 当前只运行内置 SQLx migrations，然后退出。
- `admin_roles` 和 `admin_users` 由既有 migration 创建，但 migration 不写入默认数据。
- 管理员密码使用项目现有 Argon2 `hash_password` 存储，不允许写入明文。
- 首个管理员注册接口允许空表引导，但后台 UI 只有登录入口，部署时仍需要手工准备角色。
- 完整 Compose 和 1Panel Compose 都使用一次性 `migrate` 服务阻塞 API 启动。

## Assumptions

- 默认账号为 `admin`、默认密码为 `Qaz123456@`、默认角色名为 `super_admin`。
- 部署环境变量可以覆盖三个默认值。
- 已存在任意管理员时跳过整个引导流程，不新增角色，也不修改已有管理员。

## Requirements

- 在 migrations 成功后执行管理员引导。
- 未配置环境变量时使用固定默认管理员凭据。
- 支持 `BOOTSTRAP_ADMIN_USERNAME`、`BOOTSTRAP_ADMIN_PASSWORD` 和
  `BOOTSTRAP_ADMIN_ROLE_NAME` 覆盖默认值。
- 校验管理员账号、密码和角色名称，错误配置不得写入数据库。
- 使用事务创建或复用角色，并写入 Argon2 密码哈希。
- 不在日志、错误信息或 API 容器环境中输出/保留明文密码。
- 完整 Compose、1Panel Compose、环境变量案例和部署文档同步说明初始化行为。

## Acceptance Criteria

- [x] 空管理员表时创建一个 active 管理员及对应角色。
- [x] 再次运行迁移器时跳过，不覆盖账号、角色或密码。
- [x] 已存在其他管理员时跳过，不创建额外默认账号。
- [x] 未配置覆盖变量时创建 `admin / Qaz123456@`。
- [x] 覆盖变量值非法时迁移器非零退出。
- [x] Compose 示例只把引导密码传给 `migrate`，不传给 `api`。
- [x] 相关 Rust 测试、Compose 展开检查、格式检查和编译检查通过。

## Definition of Done

- Tests added for configuration validation, first creation, idempotent skip, and password hashing.
- `cargo fmt --check` and `cargo check --all-targets` pass.
- Both Compose examples expand successfully with their example env files.
- Deployment docs and container-delivery contract describe the new variables and one-time behavior.
- `docs/superpowers/PROGRESS.md` records implementation and verification.

## Out of Scope

- 不实现管理员首次登录强制改密。
- 不修改管理员登录、注册、2FA 或权限校验接口。
- 不自动重置已有管理员密码。

## Technical Notes

- 迁移入口：`src/bin/exchange-migrate.rs`
- 密码哈希：`src/modules/auth/mod.rs::hash_password`
- 账号规范化：`src/modules/auth/mod.rs::normalize_username`
- 数据表：`migrations/0002_admin_agent_rbac.sql`
- 容器规范：`.trellis/spec/backend/container-delivery.md`
- 部署文档：`docs/deployment/docker.md`
