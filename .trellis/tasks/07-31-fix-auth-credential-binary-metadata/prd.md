# 修复认证凭据二进制元数据解码

## Goal

修复后台管理员登录读取 `admin_users.password_hash` 时，SQLx 因数据库
字段为 `VARBINARY` 或使用二进制排序规则而无法解码为 Rust `String`，
最终返回 HTTP 500 的问题，并避免用户与代理登录出现相同故障。

## What I Already Know

- 管理员登录查询为
  `SELECT id, password_hash, status FROM admin_users ...`。
- 日志中的 `column 1` 对应查询第二列 `password_hash`。
- 用户、管理员、代理凭据查询都将 `password_hash` 和 `status` 解码为
  `String`，三条路径具有同样的数据库元数据风险。
- 历史迁移 `0001`、`0002` 已经上线，不能修改其校验和。
- `0097` 已证明真实 `VARBINARY` 和 `utf8mb4_bin VARCHAR` 都会触发
  SQLx `String` 解码错误。

## Requirements

- 新增后续 SQLx 迁移，将三类登录主体的 `password_hash` 与 `status`
  明确规范化为 `utf8mb4_unicode_ci VARCHAR`。
- 保留字段长度、默认值、NULL/NOT NULL 约束和所有现有凭据值。
- 不修改密码哈希、不重置账号、不改变登录接口或令牌行为。
- 使用真实 MySQL 和生产 `MySqlAuthRepository` 查询，覆盖管理员、
  用户和代理凭据读取。
- 同时覆盖真实 `VARBINARY` 与二进制排序规则 `VARCHAR` 两种漂移。

## Acceptance Criteria

- [x] 管理员凭据查询不再因 `password_hash` 或 `status` 返回解码错误。
- [x] 用户和代理凭据查询具有相同修复效果。
- [x] Argon2 密码哈希、账号状态、默认值和字段长度完整保留。
- [x] 迁移在已经正确的 `VARCHAR` 结构上重复执行成功。
- [x] 真实 MySQL 聚焦测试、完整迁移、格式化与全目标编译通过。

## Definition of Done

- 新迁移和回归测试通过独立质量检查。
- 历史迁移保持不变。
- 规范与 `docs/superpowers/PROGRESS.md` 已更新。
- 变更提交并推送到 `main`。

## Out of Scope

- 不重构认证服务、会话或前端登录页面。
- 不修改账号标识字段和其他业务表。
- 不执行数据库范围的字符集转换。

## Technical Notes

- 管理员故障点位于
  `src/modules/auth/infrastructure.rs::find_admin_by_username`。
- 同一持久化契约还包括 `find_user_by_email/phone/username` 和
  `find_agent_by_username`。
- 追加迁移编号应为 `0098`，不得修改 `0001`、`0002` 或 `0097`。
