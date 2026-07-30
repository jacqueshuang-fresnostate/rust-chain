# Auth Session Contract

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
