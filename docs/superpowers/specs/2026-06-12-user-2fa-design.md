# 用户 2FA 与后台安全策略设计

## 目标

为 PC 用户端增加 TOTP 双重验证能力，并由 Admin 后台配置登录与资金操作的安全校验策略。用户只负责绑定、重置 2FA 以及在策略允许时开启登录 2FA；资金操作是否需要校验、需要资金密码还是 2FA，由后台统一配置。

## 已确认范围

- 2FA 方式采用 TOTP Authenticator 动态码。
- 2FA 恢复支持用户邮箱验证码自助重置，也支持 Admin 重置兜底。
- 登录 2FA 策略由后台配置：不要求、用户开启时要求、全站强制要求。
- 资金操作策略由后台按动作配置，用户不可自行关闭或修改。
- 资金操作校验方式支持：资金密码、2FA、资金密码 + 2FA。
- 如果策略要求 2FA 但用户未绑定，后端阻止操作并提示先绑定 2FA。

## 后台安全策略模型

新增后台安全策略配置，建议保存为单条 JSON 策略，便于后续追加动作。

```json
{
  "login_2fa_mode": "user_enabled",
  "payment_policies": {
    "withdraw": { "enabled": true, "method": "fund_password" },
    "spot_order": { "enabled": false, "method": "fund_password" },
    "convert": { "enabled": false, "method": "fund_password" },
    "earn_subscribe": { "enabled": false, "method": "fund_password" }
  }
}
```

### 登录策略

| 显示 | 提交值 | 行为 |
|---|---|---|
| 不要求 | `none` | 登录不触发 2FA |
| 用户开启时要求 | `user_enabled` | 用户在 PC 安全设置开启登录 2FA 后才触发 |
| 全站强制要求 | `mandatory` | 已绑定 2FA 的用户登录必须验证；未绑定用户按后端策略阻止或引导绑定 |

### 资金操作策略

| 动作 | 默认启用校验 | 默认方式 |
|---|---:|---|
| 提现 | 是 | 资金密码 |
| 闪兑 | 否 | 资金密码 |
| 现货下单 | 否 | 资金密码 |
| 理财申购 | 否 | 资金密码 |

校验方式：

- `fund_password`：仅资金密码。
- `two_factor`：仅 TOTP 2FA。
- `fund_password_and_two_factor`：资金密码 + TOTP 2FA。

## 数据库设计

### `user_two_factor_settings`

```text
user_id BIGINT PRIMARY KEY
 totp_secret_encrypted TEXT NULL
 totp_enabled BOOLEAN NOT NULL DEFAULT FALSE
 login_2fa_enabled BOOLEAN NOT NULL DEFAULT FALSE
 confirmed_at TIMESTAMP NULL
 last_verified_at TIMESTAMP NULL
 created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
 updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
```

要求：

- `totp_secret_encrypted` 使用项目现有 secret/encryption 能力加密保存。
- `totp_enabled = true` 只在用户输入正确 TOTP code 完成绑定后写入。
- Admin 重置时清空 secret，关闭 `totp_enabled` 和 `login_2fa_enabled`。

### `security_policy_configs`

```text
id BIGINT PRIMARY KEY AUTO_INCREMENT
policy_key VARCHAR(64) NOT NULL UNIQUE
policy_value JSON NOT NULL
updated_by BIGINT NULL
created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
```

建议固定 `policy_key = 'user_security_policy'`。

## 后端用户 API

| 方法 | 路径 | 用途 |
|---|---|---|
| `GET` | `/api/v1/user/2fa` | 获取用户 2FA 状态和策略摘要 |
| `POST` | `/api/v1/user/2fa/setup` | 生成 TOTP secret 和 otpauth URI |
| `POST` | `/api/v1/user/2fa/confirm` | 校验 TOTP code 并确认绑定 |
| `PATCH` | `/api/v1/user/2fa/login` | 在后台策略允许时开启/关闭登录 2FA |
| `POST` | `/api/v1/user/2fa/reset-code` | 发送邮箱验证码 |
| `POST` | `/api/v1/user/2fa/reset` | 使用邮箱验证码重置 2FA |

自助重置同时覆盖两类场景：

- 已登录用户在安全设置页重置。
- 登录过程中密码已校验通过、但卡在 2FA challenge 的用户，可通过 challenge-scoped 邮箱验证码重置 2FA；重置后需重新登录。

## 登录 2FA 流程

`POST /api/v1/auth/login` 保持原入口。

- 如果密码错误，仍返回登录失败。
- 如果密码正确且当前策略不需要 2FA，返回原 token 响应。
- 如果密码正确且需要 2FA，不返回 token，返回：

```json
{
  "requires_2fa": true,
  "challenge_id": "uuid",
  "expires_in_seconds": 300
}
```

- 如果登录策略为 `mandatory` 且用户尚未绑定 2FA，不返回完整 token，返回：

```json
{
  "requires_2fa_setup": true,
  "setup_challenge_id": "uuid",
  "expires_in_seconds": 300
}
```

该 setup challenge 只允许完成 2FA setup/confirm，成功后签发正式 token 或要求用户重新登录；不允许访问其他用户接口。

新增：

| 方法 | 路径 | 用途 |
|---|---|---|
| `POST` | `/api/v1/auth/login/2fa` | 校验 `challenge_id + totp_code`，通过后签发 token |
| `POST` | `/api/v1/auth/login/2fa/reset-code` | 登录 challenge 场景发送 2FA 重置邮箱验证码 |
| `POST` | `/api/v1/auth/login/2fa/reset` | 登录 challenge 场景用邮箱验证码重置 2FA，重置后要求重新登录 |

challenge 应短期有效，建议 5 分钟过期，并在成功使用后失效。

## 资金操作校验

新增后端公共 helper，用于资金动作执行前校验安全策略：

```text
verify_user_security_action(user_id, action, fund_password, totp_code)
```

行为：

- 策略未启用：直接通过。
- 要求资金密码：校验 `fund_password`。
- 要求 2FA：校验 `totp_code`。
- 要求双校验：两个都必须正确。
- 策略要求 2FA 但用户未绑定：拒绝并提示先绑定 2FA。
- 策略要求资金密码但用户未设置：拒绝并提示先设置资金密码。

第一期必须接入提现。现货下单、闪兑、理财申购先保留统一策略结构；对应后端接口成熟后按同一 helper 接入。

业务请求可统一携带：

```json
{
  "fund_password": "xxxx",
  "totp_code": "123456"
}
```

## 后台 API

| 方法 | 路径 | 用途 |
|---|---|---|
| `GET` | `/admin/api/v1/security-policy` | 获取安全策略 |
| `PATCH` | `/admin/api/v1/security-policy` | 更新登录和资金操作策略 |
| `POST` | `/admin/api/v1/users/:id/2fa/reset` | 管理员重置指定用户 2FA |

后台更新策略和重置用户 2FA 都写入 `admin_audit_logs`。

## PC 用户端设计

在 `用户中心 / 安全设置` 增加 “双重验证 2FA” 区块。

### 状态展示

| 状态 | 展示 |
|---|---|
| 未绑定 | 未绑定 2FA，显示绑定按钮 |
| 已绑定 | 已绑定 2FA，显示重置按钮 |
| 登录策略为 `user_enabled` | 显示登录 2FA 开关 |
| 登录策略为 `none` | 不显示开关，提示当前未要求登录 2FA |
| 登录策略为 `mandatory` | 不显示关闭入口，提示平台要求登录验证 2FA |

### 绑定流程

1. 用户点击绑定。
2. 请求 `/user/2fa/setup`。
3. 弹窗显示二维码 URI 和手动密钥。
4. 用户用 Authenticator 扫码。
5. 用户输入 6 位动态码。
6. 请求 `/user/2fa/confirm`。
7. 成功后刷新安全设置。

### 重置流程

1. 用户点击重置 2FA。
2. 请求 `/user/2fa/reset-code` 发送邮箱验证码。
3. 用户输入邮箱验证码。
4. 请求 `/user/2fa/reset`。
5. 成功后清空绑定状态，用户可重新绑定。

### 登录流程

- 普通 token 响应沿用现有逻辑。
- 如果返回 `requires_2fa + challenge_id`，登录页切换为 2FA 验证步骤。
- 用户输入 TOTP code 后请求 `/auth/login/2fa`。
- 成功后保存 token、加载 profile、进入首页。

### 资金操作流程

- 资金动作提交前，根据策略摘要显示校验弹窗。
- 校验弹窗按策略显示资金密码、2FA code 或两个输入框。
- 提交业务请求时带上对应字段。
- 如果后端返回未绑定 2FA，提示并引导到安全设置页。
- 如果登录返回 `requires_2fa_setup`，PC 端显示强制绑定步骤，只允许完成 2FA 绑定后继续登录或重新登录。

## Admin 前端设计

新增页面：

```text
系统配置 / 安全策略
```

页面包含：

1. 登录 2FA 策略下拉。
2. 资金操作策略表格。

| 操作 | 是否启用额外校验 | 校验方式 |
|---|---|---|
| 提现 | 是/否 | 资金密码 / 2FA / 资金密码+2FA |
| 闪兑 | 是/否 | 资金密码 / 2FA / 资金密码+2FA |
| 现货下单 | 是/否 | 资金密码 / 2FA / 资金密码+2FA |
| 理财申购 | 是/否 | 资金密码 / 2FA / 资金密码+2FA |

在 Admin 用户管理详情或行级操作中增加：

```text
重置 2FA
```

调用 `POST /admin/api/v1/users/:id/2fa/reset`。

## 错误处理

| 场景 | 错误码 | 消息 |
|---|---|---|
| TOTP code 错误 | `invalid_2fa_code` | 2FA 验证码错误 |
| 重复绑定 | `2fa_already_enabled` | 2FA 已绑定 |
| 未绑定却开启登录 2FA | `2fa_not_enabled` | 请先绑定 2FA |
| 策略不允许用户控制登录 2FA | `login_2fa_policy_locked` | 当前登录 2FA 策略不允许用户修改 |
| 操作要求 2FA 但用户未绑定 | `2fa_required_not_bound` | 请先绑定 2FA |
| 操作要求资金密码但用户未设置 | `fund_password_required_not_set` | 请先设置资金密码 |
| 缺少安全校验字段 | `security_verification_required` | 请完成安全校验 |
| 登录 challenge 过期 | `login_2fa_challenge_expired` | 登录验证已过期，请重新登录 |

## 测试计划

### 后端

- 2FA setup 生成 secret 和 otpauth URI。
- confirm 只有正确 TOTP code 才能完成绑定。
- `/user/2fa` 返回绑定状态和策略摘要。
- 登录策略为 `user_enabled` 且用户开启登录 2FA 时，登录返回 challenge，不直接签发 token。
- `/auth/login/2fa` 使用正确 challenge 和 TOTP code 后签发 token。
- 登录策略为 `mandatory` 且用户未绑定时，登录阻止或返回必须绑定 2FA 的状态。
- 提现默认要求资金密码。
- 提现配置为 2FA 时，未绑定用户被阻止。
- 提现配置为双校验时，资金密码和 TOTP 都必须正确。
- Admin 获取/更新策略写入 audit log。
- Admin 重置用户 2FA 生效。
- OpenAPI 暴露新增接口。

### PC

- backend adapters 映射 2FA 状态、登录 challenge 和安全策略。
- 登录页普通 token 登录沿用旧流程。
- 登录页在 requires 2FA 时显示二阶段验证。
- 安全设置页未绑定显示绑定入口，已绑定显示重置入口。
- 不同登录策略下登录 2FA 开关显示正确。
- 资金操作弹窗按策略显示资金密码、2FA 或双输入。

### Admin

- 安全策略配置页渲染默认策略。
- 修改登录策略和资金操作策略提交正确 payload。
- 用户行级重置 2FA 调用正确接口。
- 路由和侧边栏入口存在。

## 实施边界

- 第一阶段必须完成用户 2FA、登录 2FA、Admin 策略配置、Admin 重置、PC 安全设置、PC 登录二阶段验证、提现安全校验闭环。
- 现货下单、闪兑、理财申购先建立策略结构和前端弹窗复用能力；如果对应后端接口当前已稳定，再按同一 helper 接入。
- 不引入短信 2FA。
- 不允许用户自行关闭后台要求的资金操作校验。
- 不提交代码，除非用户明确要求。
