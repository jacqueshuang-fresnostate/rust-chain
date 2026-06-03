# 08. 用户认证与安全 API

本文档记录用户注册、登录、邮箱绑定、登录密码、资金密码和后台 SMTP 配置 API。所有时间字段均为 Unix milliseconds。本文档对应接口已纳入首批 OpenAPI，入口为 `GET /openapi.json`、`GET /docs`，兼容入口为 `GET /api/openapi.json`、`GET /api/docs`。

## 1. 通用约定

### 1.1 地址前缀

| API 类型 | 前缀 |
|---|---|
| 用户端 | `/api/v1` |
| 管理后台 | `/admin/api/v1` |

### 1.2 鉴权

| 鉴权类型 | Header | 适用接口 |
|---|---|---|
| 无 | 无 | 用户注册、用户登录 |
| UserAuth | `Authorization: Bearer <user_access_token>` | 用户 profile、邮箱绑定、登录密码、资金密码 |
| Refresh token | 请求体 `refresh_token` | 用户 token 刷新 |
| AdminAuth | `Authorization: Bearer <admin_access_token>` | 后台 SMTP 查询、保存、测试发送 |

UserAuth、AdminAuth、AgentAuth 不可混用。用户安全接口收到 admin 或 agent token 返回 `FORBIDDEN`。

### 1.3 Token 响应

```json
{
  "access_token": "<jwt>",
  "refresh_token": "<refresh-token>",
  "token_type": "Bearer",
  "scope": "user"
}
```

### 1.4 错误响应

```json
{
  "code": "VALIDATION_ERROR",
  "message": "validation error: reason is required"
}
```

| HTTP | code | 场景 |
|---|---|---|
| 400 | `VALIDATION_ERROR` | 参数缺失、邮箱格式错误、验证码错误、密码规则不满足 |
| 401 | `UNAUTHORIZED` | 未登录、token 无效、旧密码错误、登录密码错误、旧资金密码错误、用户已禁用 |
| 403 | `FORBIDDEN` | 鉴权 scope 不匹配 |
| 404 | `NOT_FOUND` | 资源不存在或未配置可用 SMTP |
| 409 | `CONFLICT` | 邮箱被其他用户占用、资金密码重复新建 |
| 500 | `INTERNAL_ERROR` | MySQL、SMTP sender、加密 key 或服务配置缺失 |

## 2. 用户认证 API

### 2.1 注册

`POST /api/v1/auth/register`

请求：

```json
{
  "email": "user@example.test",
  "phone": null,
  "password": "password-1"
}
```

说明：`email` 与 `phone` 至少提供一个。注册成功后返回 user scope token。

响应：见 [Token 响应](#13-token-响应)。

### 2.2 登录

`POST /api/v1/auth/login`

请求：

```json
{
  "email": "user@example.test",
  "phone": null,
  "password": "password-1"
}
```

响应：见 [Token 响应](#13-token-响应)。

### 2.3 刷新 token

`POST /api/v1/auth/refresh`

请求：

```json
{
  "refresh_token": "<refresh-token>"
}
```

响应：见 [Token 响应](#13-token-响应)。

## 3. 用户安全 API

### 3.1 查询个人信息

`GET /api/v1/user/profile`

鉴权：UserAuth。

响应：

```json
{
  "id": 1,
  "email": "user@example.test",
  "phone": null,
  "status": "active",
  "kyc_level": 0,
  "email_verified_at": 1780428000000,
  "fund_password_set": true,
  "created_at": 1780427000000
}
```

字段说明：

| 字段 | 类型 | 说明 |
|---|---|---|
| `email_verified_at` | number \| null | 邮箱验证时间；未验证为 `null` |
| `fund_password_set` | boolean | 是否已设置资金密码 |

### 3.2 发送绑定邮箱验证码

`POST /api/v1/user/email/bind-code`

鉴权：UserAuth。

请求：

```json
{
  "email": "user@example.test"
}
```

响应：

```json
{
  "sent": true,
  "expires_at": 1780428600000
}
```

规则：

- 用户必须为 `active`。
- 邮箱格式必须合法，且不能被其他用户占用。
- 同一用户同一邮箱 60 秒内重复发送会被拒绝。
- 验证码为 6 位数字，10 分钟内有效。
- 系统只保存验证码 hash，不在响应、日志或审计中返回验证码。
- 必须已配置并启用后台 SMTP，且服务端已注入邮件 sender。

### 3.3 绑定邮箱

`POST /api/v1/user/email/bind`

鉴权：UserAuth。

请求：

```json
{
  "email": "user@example.test",
  "code": "123456"
}
```

响应：

```json
{
  "email": "user@example.test",
  "email_verified_at": 1780428200000
}
```

规则：

- 用户必须为 `active`。
- 只校验最新 pending 绑定邮箱验证码。
- 验证码过期、错误或超过 5 次尝试返回 `VALIDATION_ERROR`。
- 错误验证码会持久化增加尝试次数。
- 成功后事务内更新 `users.email`、`users.email_verified_at`，并将验证码状态置为 verified。
- 写入用户审计事件，审计内容不包含验证码。

### 3.4 修改登录密码

`PATCH /api/v1/user/password`

鉴权：UserAuth。

请求：

```json
{
  "old_password": "password-1",
  "new_password": "password-2"
}
```

响应：见 [Token 响应](#13-token-响应)。

规则：

- 旧登录密码必须正确。
- 新登录密码至少 8 位，且不能与旧密码相同。
- 成功后更新 `users.password_hash`，吊销该用户所有未吊销 refresh token，并签发新的 user scope token。
- 客户端必须用响应中的新 token 替换本地会话。

### 3.5 新建资金密码

`POST /api/v1/user/fund-password`

鉴权：UserAuth。

请求：

```json
{
  "login_password": "password-1",
  "fund_password": "123456"
}
```

响应：

```json
{
  "fund_password_set": true
}
```

规则：

- 必须验证当前登录密码。
- 资金密码必须为 6 位数字。
- 资金密码不能与登录密码相同。
- 已设置资金密码时重复新建返回 `CONFLICT`。
- 资金密码只保存 hash，不返回明文。

### 3.6 修改资金密码

`PATCH /api/v1/user/fund-password`

鉴权：UserAuth。

请求：

```json
{
  "old_fund_password": "123456",
  "new_fund_password": "654321"
}
```

响应：

```json
{
  "fund_password_set": true
}
```

规则：

- 必须已设置资金密码。
- 旧资金密码必须正确。
- 新资金密码必须为 6 位数字，且不能与旧资金密码相同。
- 资金密码只保存 hash，不返回明文。

## 4. 后台 SMTP API

### 4.1 查询 SMTP 配置

`GET /admin/api/v1/smtp/config`

鉴权：AdminAuth。

响应：

```json
{
  "id": 1,
  "name": "default",
  "host": "smtp.example.test",
  "port": 587,
  "security": "starttls",
  "username_mask": "mail****user",
  "password_set": true,
  "from_email": "noreply@example.test",
  "from_name": "Exchange",
  "enabled": true
}
```

未保存配置时响应为 `null`。

### 4.2 保存 SMTP 配置

`PATCH /admin/api/v1/smtp/config`

鉴权：AdminAuth。

请求：

```json
{
  "host": "smtp.example.test",
  "port": 587,
  "security": "starttls",
  "username": "mail-user",
  "password": "smtp-password",
  "from_email": "noreply@example.test",
  "from_name": "Exchange",
  "enabled": true,
  "reason": "configure smtp"
}
```

响应：见 [查询 SMTP 配置](#41-查询-smtp-配置) 的对象格式。

规则：

- `reason` 必填，最长 512 字符。
- `host`、`from_email` 必填；`port` 必须为 `1..=65535`。
- `security` 只能为 `none`、`starttls`、`tls`。
- `username` 与 `password` 必须成对配置；更新时空密码表示保留旧密文。
- username/password 加密存储；响应和审计只返回 `username_mask`、`password_set`。
- 写入 `admin_audit_logs`，action 为 `smtp_config.save`。

### 4.3 发送 SMTP 测试邮件

`POST /admin/api/v1/smtp/test`

鉴权：AdminAuth。

请求：

```json
{
  "recipient": "ops@example.test",
  "reason": "verify smtp"
}
```

响应：

```json
{
  "sent": true,
  "recipient": "ops@example.test"
}
```

规则：

- `reason` 必填，最长 512 字符。
- 必须存在 enabled 的默认 SMTP 配置。
- 发送前写入 `admin_audit_logs`，action 为 `smtp_config.test`，审计不包含明文或密文。

## 5. 安全说明

- 登录密码、资金密码、邮箱验证码均以 hash 形式持久化。
- SMTP username/password 使用 32 字节 `credential_encryption_key` 加密存储。
- SMTP 响应、审计和前端页面不得展示密码明文或密文。
- 修改登录密码会吊销旧 refresh token，避免旧会话继续刷新。
- 邮箱验证码错误次数持久化，防止反复尝试绕过限制。
- 后台高风险写操作必须提交非空 `reason`，并写入审计日志。
