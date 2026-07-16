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
- Redis/session backend failure during validation may return an internal error; do not silently accept the token.

### 5. Good/Base/Bad Cases

- Good: user login creates a sa-token access token with login type `user`, stores refresh metadata in Redis, and PC keeps sending `Authorization: Bearer ...`.
- Base: tests without `auth_manager` may still use `issue_token(settings, "user:42", TokenScope::User, 900)` for legacy extractor coverage.
- Bad: refreshing a user token creates a sa-token access token with login type `default`, causing it to fail `UserAuth`.
- Bad: changing a password only updates MySQL and leaves old Redis refresh tokens usable.

### 6. Tests Required

- Auth unit tests must cover:
  - sa-token access token accepted by the existing extractors.
  - scope mismatch remains forbidden.
  - refresh preserves `Claims.sub` legacy subject shape.
- WebSocket tests must cover `/ws/private?token=...` for valid user tokens and reject non-user scopes.
- Frontend PC/admin tests must cover Bearer header injection and login response persistence when auth payload fields stay unchanged.
- Frontend request-layer tests must cover one-shot refresh retry for protected routes and no recursive refresh retry for auth bootstrap routes.
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
