# 用户用户名配置与用户名登录开关

## Goal

让用户可以在 PC 安全中心配置唯一用户名，并允许后台通过安全策略开关控制是否开放“用户名 + 密码”登录。邮箱/手机号登录保持兼容，用户名登录默认关闭，避免上线后改变既有登录面。

## Requirements

* 后端 `users` 增加可空 `username` 字段，并通过唯一索引保证用户名唯一。
* 用户可通过已登录接口设置或修改自己的用户名；用户名需要标准化、校验格式，并在冲突时返回明确错误。
* 用户资料接口返回真实 `username`，PC 用户中心/Header 优先展示真实用户名，未配置时继续降级显示邮箱/手机号。
* 后台用户安全策略增加“允许用户名登录”开关，和现有登录 2FA、邀请码、第三方绑定策略一起保存、审计、展示。
* 用户登录接口支持 `username` 字段；只有后台开关开启时才允许通过用户名查找账号登录。
* PC 登录页在开关开启时允许输入邮箱或用户名；关闭时维持邮箱登录展示和请求。
* OpenAPI、管理端、PC 类型与本地化文案同步更新。

## Acceptance Criteria

* [ ] 新迁移可以为 `users.username` 建唯一索引，并补齐安全策略 JSON 的默认 `username_login_enabled=false`。
* [ ] 后端用户名校验覆盖空值、非法字符、长度、大小写标准化与重复用户名。
* [ ] `/api/v1/user/profile` 返回 `username`，`PATCH /api/v1/user/username` 可更新并返回标准化用户名。
* [ ] `/api/v1/auth/login` 在开关关闭时拒绝用户名登录，在开关开启时支持用户名登录；邮箱/手机号逻辑不受影响。
* [ ] 管理后台安全策略页可开启/关闭用户名登录，并随 PATCH 请求提交。
* [ ] PC 登录页、用户安全中心有对应文案和 API 对接。

## Definition of Done

* 后端格式化与相关 Rust 测试通过。
* PC 类型检查与相关静态/单元测试通过。
* Web 管理端类型检查/相关测试通过。
* `docs/superpowers/PROGRESS.md` 记录本次交付。

## Technical Approach

使用现有安全策略 JSON 扩展 `username_login_enabled`，避免新增独立配置表。用户登录仍走 `AuthService::verify_user_credentials`，在 `UserCredentials` 中增加用户名与允许用户名登录标记；数据库仓储增加 `find_user_by_username`。用户侧新增 `PATCH /user/username`，复用用户审计事件。PC 安全中心增加用户名配置卡片和弹窗，登录页加载登录配置后决定是否提交 `username`。

## Decision (ADR-lite)

**Context**: 项目已有 `security_policy_configs.policy_value` 保存用户安全策略，注册邀请码、2FA、第三方绑定都通过同一对象配置。

**Decision**: 将用户名登录开关作为 `UserSecurityPolicy.username_login_enabled` 的一部分；用户名本身落在 `users.username`，API 层统一标准化为小写字母数字下划线。

**Consequences**: 策略读取和审计保持一致；默认关闭保证兼容。用户名显示和登录名会被标准化，后续若需要展示名/昵称，需要另加 display name 字段而不是复用登录用户名。

## Out of Scope

* 不做用户名找回密码。
* 不做用户名修改冷却时间或历史用户名保留。
* 不把注册流程改成必须填写用户名。
* 不支持中文用户名，避免登录标识大小写、归一化和混淆问题。

## Technical Notes

* 相关后端文件：`src/modules/auth/mod.rs`、`src/modules/auth/routes.rs`、`src/modules/security.rs`、`src/modules/user/routes.rs`、`src/modules/admin/routes.rs`、`src/openapi.rs`。
* 相关 PC 文件：`pc/src/api/auth.ts`、`pc/src/api/user.ts`、`pc/src/api/backendAdapters.ts`、`pc/src/views/auth/Login.vue`、`pc/src/views/User/Security.vue`、`pc/src/i18n/index.ts`。
* 相关管理端文件：`web/src/admin/actions/SecurityPolicyPage.tsx`、`web/src/admin/actions/SecurityPolicyPage.test.tsx`。
* 现有 `security_policy_configs` 为 JSON 策略，`UserSecurityPolicy` 使用 serde default 可兼容旧 JSON。
