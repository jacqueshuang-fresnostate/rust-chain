# sa-token-rust Redis Research

## Sources Checked

- GitHub repository: `https://github.com/sa-tokens/sa-token-rust` cloned on 2026-06-17.
- Crates.io metadata checked with `cargo info sa-token-core`, `cargo info sa-token-plugin-axum`, and `cargo info sa-token-storage-redis`.
- Local upstream files inspected under `/tmp/sa-token-rust`: `README.md`, `doc/guide/storage.md`, `sa-token-core/src/manager.rs`, `sa-token-core/src/config.rs`, `sa-token-core/src/refresh.rs`, `sa-token-storage-redis/src/lib.rs`.

## Relevant Current Version

- Current crates.io release is `0.1.18`.
- `sa-token-core = "0.1.18"` provides `SaTokenManager`, `SaTokenConfig`, `TokenValue`, `TokenInfo`, and multi-account `login_type`.
- `sa-token-storage-redis = "0.1.18"` provides `RedisStorage::new(redis_url, key_prefix).await`.
- `sa-token-plugin-axum = "0.1.18"` defaults to `axum-08`.

## Findings

- The Axum plugin is not a clean fit because this repo currently uses Axum `0.7`; pulling the plugin would risk a framework-wide upgrade. Use `sa-token-core` directly instead.
- `SaTokenManager::login_with_options(login_id, Some(login_type), device, extra, nonce, expire)` writes access token metadata into configured storage and supports Redis through `SaStorage`.
- `SaTokenManager::get_token_info(TokenValue)` validates existence, expiration, kick-out/replaced markers, and active timeout.
- `SaTokenConfig` supports `storage_key_prefix`, `timeout`, `token_style`, `is_concurrent`, `is_share`, `auto_renew`, and refresh-token fields.
- `sa-token-storage-redis::RedisStorage` opens its own Redis connection using a URL and applies a physical key prefix. `SaTokenConfig.storage_key_prefix` is a logical prefix used by core token keys.
- Built-in `RefreshTokenManager::refresh_access_token` creates a new `TokenInfo` with default `login_type`, so using it directly would break this repo's user/admin/agent scope isolation.

## Recommended Integration

- Add `sa-token-core`, `sa-token-storage-redis`, and `sa-token-storage-memory`.
- Store an `Arc<SaTokenManager>` in `AppState`.
- Build the manager at startup with Redis storage:
  - physical Redis prefix: `exchange:sa-token:`
  - logical sa-token prefix: `auth:`
  - token style: `Random64`
  - access timeout: existing `jwt_access_ttl_seconds`
  - no auto-renew for now, to preserve current fixed access-token TTL behavior.
- For unit tests without Redis, provide a memory-backed manager helper.
- Implement project-specific refresh records in sa-token storage or existing Redis:
  - key by refresh token
  - include actor_type, actor_id, user_id, scope, subject, expires_at
  - index refresh tokens by actor for password-change revocation
- Keep frontend payloads unchanged and continue using `Authorization: Bearer <token>`.

## Risks

- This is a behavioral auth migration: old JWT access tokens and MySQL refresh tokens will no longer validate after deployment.
- Existing tests generate tokens with `issue_token(...)`; that helper must either create sa-token sessions in a provided manager or tests that hit extractors need memory-backed auth state.
- The Redis storage crate depends on redis 1.x while this repo currently uses redis 0.27. Multiple versions can coexist, but code should avoid mixing manager types between them.
