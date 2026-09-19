# Auth Session Contract

## 用户与代理会话代际

### 1. 范围

用户/代理 HTTP、私有 WebSocket、登录、刷新、登录 2FA、改密、找回密码及后台停用。

### 2. 接口与存储

- 迁移 `0130_user_agent_session_versions.sql` 为 `users`、`agent_admin_users`、`login_two_factor_challenges` 增加 `auth_session_version`，默认 0。
- 凭据查询同一快照读取密码、状态和代际；JWT、Sa-Token extra、刷新记录及 2FA 挑战都携带该代际。
- `claims_from_bearer_token` 同时校验状态和代际；代理还回查自身及全部祖先状态。

### 3. 合同

- 用户停用与改密、代理改密都在现有业务事务内增加代际；代理停用增加整个子树门户账号的代际，不改写子节点状态。
- 重新启用不降低代际；外部会话撤销失败不影响数据库闸门，旧会话仍然无效。
- 签发只能使用认证证明携带的代际，不得读取新代际后给旧密码证明或旧 2FA 挑战升级。
- 改密后签发使用锁定行原代际加一；并发再次变更时拒绝签发，要求重新登录。
- 私有 WS 在升级、确认、转发前校验令牌；空闲连接每 5 秒复核，存储出错即关闭，不继续发送私有消息。
- 数据库未挂载的轻量测试保持兼容；已配置数据库但不可用时禁止退化为只验签。

### 4. 错误矩阵

停用、代际不匹配、主体不存在：401；错误 scope：403；数据库或会话存储故障：失败关闭。旧 2FA 挑战按过期处理。

### 5. 正反例

正确：停用后启用，新登录成功而旧访问/刷新令牌失败。基础：迁移前代际 0 的活跃会话在首次安全变更前兼容。错误：只删除 Redis 而不递增数据库代际。

### 6. 必要测试

验证用户及代理子树停用、重新启用、旧刷新记录存活、旧密码证明拒签、新登录成功、私有 WS 已连接后代际变化关闭且不转发；密码变更及非零代际 2FA 必须覆盖。

### 7. 错误与正确

错误：`issue_tokens(old_proof.with_version(load_current_version()))`。
正确：签发保留 `old_proof.version`，与当前数据库版本不等就拒绝。

## Scenario: sa-token-rust Redis-backed sessions

### 1. Scope / Trigger

- Trigger: any change to login, register, 2FA login, refresh, request extractors, password-change revocation, or `/ws/private` token validation.
- Scope: `src/modules/auth`, `src/modules/events`, `src/modules/user`, `src/infra/auth`, and frontend clients that persist or send auth tokens.

### 2. Signatures

- Runtime session manager: `AppState.auth_manager: Option<Arc<SaTokenManager>>`.
- Runtime initialization: `infra::auth::connect(settings)` must build a Redis-backed `SaTokenManager`.
- Test initialization: `infra::auth::memory_manager(settings)` may build an in-memory manager without initializing global `StpUtil`.
- HTTP token format remains:
  - request header: `Authorization: Bearer <access_token>`
  - private websocket query: `/ws/private?token=<access_token>`
- First-login mandatory TOTP enrollment uses public challenge routes:
  ```text
  POST /api/v1/auth/login/2fa/setup
  {"setup_challenge_id":"..."}

  POST /api/v1/auth/login/2fa/setup/confirm
  {"setup_challenge_id":"...","totp_code":"123456"}
  ```
- Setup returns `secret`, `otpauth_uri`, and `expires_in_seconds`; confirm returns
  the standard user token response.
- Login/refresh response fields remain:
  ```json
  {
    "access_token": "...",
    "refresh_token": "...",
    "token_type": "Bearer",
    "scope": "user"
  }
  ```

### 3. Contracts

- Main runtime must inject a Redis-backed `auth_manager`; production request validation must prefer sa-token session storage over legacy JWT decoding.
- User, admin, and agent sessions must use separate sa-token `login_type` values: `user`, `admin`, and `agent`.
- Business route `Claims.sub` must keep the legacy shape: `user:<id>`, `admin:<id>`, or `agent:<id>`.
- `UserAuth`, `AdminAuth`, and `AgentAuth` must reject missing/invalid tokens with 401 and wrong scopes with 403.
- Refresh tokens are project-owned Redis records keyed by a digest of the refresh token. They must store actor type, actor id, user id, scope, and expiration.
- Do not use sa-token-core `RefreshTokenManager::refresh_access_token` directly unless it preserves `login_type`; version 0.1.18 refreshes into default login type and breaks scope isolation.
- Password changes must revoke old user refresh sessions and old sa-token access sessions before returning a new token pair.
- Frontend PC/admin/agent clients should continue storing `access_token`/`refresh_token` and sending Bearer headers; do not require UI rewrites for the sa-token migration.
- Frontend clients should retry protected API requests once after a 401 by calling the matching `/auth/refresh` route with the stored `refresh_token`, updating local tokens, and replaying the original request. Login, register, 2FA, and refresh routes must not recursively trigger this retry. If refresh fails, clear local login state and require the user to log in again.
- A `setup_2fa` login challenge must expose its TOTP secret only through the
  dedicated setup route. The initial login challenge response contains only
  `requires_2fa_setup`, `setup_challenge_id`, and expiry metadata.
- Setup must validate an unexpired, unconsumed `setup_2fa` challenge, generate a
  new secret, encrypt it with the credential key, and persist it as pending for
  the challenge user.
- Confirm must validate the pending secret and TOTP code, enable user TOTP,
  atomically consume the same challenge, and only then issue a standard
  `scope=user` token pair. Invalid codes must not consume the challenge.
- Legacy JWT decoding is allowed only when `AppState.auth_manager` is absent, for lightweight tests that intentionally do not initialize auth session state.

### 4. Validation & Error Matrix

- Missing Bearer header -> 401.
- sa-token access token missing from Redis -> 401.
- sa-token token expired, kicked out, replaced, inactive, empty, or too short -> 401.
- Token `login_type` does not match the extractor scope -> 403.
- Refresh token not found or expired -> 401.
- Frontend refresh retry failure -> clear the local session and redirect to login.
- Refresh token scope mismatch -> 401.
- Refresh actor no longer active -> 401.
- Missing, expired, consumed, or wrong-type setup challenge -> 400 security error.
- Setup confirm without a pending secret -> 400 `security_verification_required`.
- Invalid setup TOTP code -> 400 `invalid_2fa_code`; challenge remains usable.
- Replayed setup confirm -> 400 `login_2fa_challenge_expired`; no second token pair.
- Redis/session backend failure during validation may return an internal error; do not silently accept the token.

### 5. Good/Base/Bad Cases

- Good: user login creates a sa-token access token with login type `user`, stores refresh metadata in Redis, and PC keeps sending `Authorization: Bearer ...`.
- Good: a mandatory first-login setup challenge generates a QR secret, accepts
  the current TOTP code once, enables TOTP, consumes the challenge, and returns
  the normal user token payload.
- Base: tests without `auth_manager` may still use `issue_token(settings, "user:42", TokenScope::User, 900)` for legacy extractor coverage.
- Base: an invalid TOTP code leaves the setup challenge and pending secret
  available for another attempt before expiry.
- Bad: refreshing a user token creates a sa-token access token with login type `default`, causing it to fail `UserAuth`.
- Bad: changing a password only updates MySQL and leaves old Redis refresh tokens usable.
- Bad: returning the TOTP secret in the initial login response or issuing tokens
  before atomically consuming the setup challenge allows secret disclosure or
  replay.

### 6. Tests Required

- Auth unit tests must cover:
  - sa-token access token accepted by the existing extractors.
  - scope mismatch remains forbidden.
  - refresh preserves `Claims.sub` legacy subject shape.
- WebSocket tests must cover `/ws/private?token=...` for valid user tokens and reject non-user scopes.
- Frontend PC/admin tests must cover Bearer header injection and login response persistence when auth payload fields stay unchanged.
- Frontend request-layer tests must cover one-shot refresh retry for protected routes and no recursive refresh retry for auth bootstrap routes.
- Real-MySQL route tests must cover setup response fields, invalid-code
  non-consumption, successful enablement/token issuance, wrong-type/expired/
  consumed challenges, and replay rejection.
- OpenAPI tests must register both setup routes and schemas while asserting that
  the initial challenge response does not expose `secret` or `otpauth_uri`.
- Run `cargo check --all-targets` after auth contract changes because many modules destructure `UserAuth/AdminAuth/AgentAuth`.

### 7. Wrong vs Correct

Wrong:

```rust
let (new_access, _) = RefreshTokenManager::new(storage, config)
    .refresh_access_token(refresh_token)
    .await?;
```

Correct:

```rust
let access = manager
    .login_with_options(actor_id, Some(scope.as_login_type().to_owned()), Some("api".to_owned()), extra, None, None)
    .await?;
```

Wrong:

```rust
LoginResponse { setup_challenge_id, secret, otpauth_uri }
```

Correct:

```rust
LoginResponse { requires_2fa_setup: true, setup_challenge_id, expires_in_seconds }
// The secret is returned only by POST /auth/login/2fa/setup after challenge validation.
```

## Scenario: SQLx-compatible auth credential text metadata

### 1. Scope / Trigger

- Trigger: changes to `users`, `admin_users`, or `agent_admin_users`
  credential columns, migrations, or `MySqlAuthRepository` credential lookups.
- Scope: user email/phone/username login, admin username login, and agent
  username login.

### 2. Signatures

- `password_hash VARCHAR(255) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NOT NULL`
- `status VARCHAR(32) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NOT NULL DEFAULT 'active'`
- Production read shape:
  `SELECT id, password_hash, status ... -> (u64, String, String)`.

### 3. Contracts

- All three actor tables own password hashes and statuses as text, never binary
  application values.
- Metadata repair must preserve exact Argon2 hash strings, existing status
  values, lengths, nullability, and the `active` status default.
- A repair changes only column metadata. It must not change login identifiers,
  status policy, password verification, token issuance, or session behavior.

### 4. Validation & Error Matrix

- `VARBINARY` credential column -> production repository returns
  `AppError::Database(sqlx::Error::ColumnDecode { .. })`.
- `utf8mb4_bin VARCHAR` credential column -> same decode failure under the
  supported SQLx/MySQL boundary.
- Invalid UTF-8 stored bytes -> migration failure; do not replace or reset the
  credential.
- Correct `utf8mb4_unicode_ci VARCHAR` metadata -> all credential lookups
  decode into `StoredActorCredential`.

### 5. Good/Base/Bad Cases

- Good: repair all six credential columns in a new immutable migration and
  verify user, admin, and agent lookups with their original hashes/statuses.
- Base: run the repair SQL against already-correct metadata without changing
  values or defaults.
- Bad: cast each login query to `CHAR`, decode into `Vec<u8>`, or reset
  password hashes to avoid fixing the schema drift.

### 6. Tests Required

- Use real MySQL 8.4 and the production `MySqlAuthRepository`.
- Reproduce both real `VARBINARY` and `utf8mb4_bin VARCHAR` metadata before
  executing the migration through `include_str!`.
- Assert all three user lookup identifiers plus admin and agent lookup fail at
  `ColumnDecode` index `1` (`password_hash`) before repair and succeed after
  repair.
- Assert exact Argon2 hashes, password verification, non-active status values,
  default `active` statuses, lengths, nullability, character set, and collation.
- Run the full SQLx migration set twice and keep historical migrations
  unchanged.

### 7. Wrong vs Correct

Wrong:

```rust
let row = sqlx::query_as::<_, (u64, Vec<u8>, Vec<u8>)>(
    "SELECT id, password_hash, status FROM users WHERE email = ? LIMIT 1",
)
.fetch_optional(pool)
.await?;
```

Correct:

```sql
ALTER TABLE users
    MODIFY COLUMN password_hash VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NOT NULL,
    MODIFY COLUMN status VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active';
```
