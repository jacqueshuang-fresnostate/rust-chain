# Market Feed Admin Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Admin-managed third-party market feed subscription configuration with MySQL persistence, encrypted provider credentials, manual reload, and React Admin frontend integration.

**Architecture:** Add dedicated MySQL tables for subscription config and provider credentials, then expose AdminAuth-protected APIs in the existing Admin route module. Runtime uses a small in-process market feed supervisor handle stored on `AppState`; saving config only increments a DB version, while `POST /market-feed/reload` explicitly applies it. Frontend adds a focused Admin action page under 行情市场 that edits config, manages masked credentials, and triggers reload with reason confirmation.

**Tech Stack:** Rust 2024, Axum, SQLx/MySQL, ring AEAD encryption, Tokio, React, TypeScript, Vite, Semi Design, Vitest, Testing Library.

---

## Current Evidence

- Spec is saved at `docs/superpowers/specs/2026-05-30-market-feed-admin-config-design.md`.
- Existing env-driven startup passes `state.settings.market_feed_symbols`, `market_feed_intervals`, and `market_feed_providers` to `market_feed::run_loop` in `src/main.rs`.
- `src/workers/market_feed.rs` already has `MarketFeedRuntimeConfig`, `run_config_loop`, and validation through `MarketFeedWorker::<MarketIngestionService>::provider_configs_for`.
- Existing Admin routes live in `src/modules/admin/routes.rs`; this file already has AdminAuth, audit helpers, Unix-millisecond serializers, and MySQL query patterns.
- Existing admin integration tests live in `tests/admin_routes.rs` and run migrations with `sqlx::migrate!("./migrations")`.
- Existing Admin frontend route map is `web/src/admin/routes.tsx`; sidebar nav is `web/src/layouts/AdminLayout.tsx`.
- Existing action-page pattern is `web/src/admin/actions/MarketStrategyActions.tsx`; confirmation modal is `web/src/shared/ConfirmAction.tsx`.
- Current project is not a git repository; replace commit steps with progress-record updates.

## File Structure

### Backend

- Modify `Cargo.toml`
  - Add direct dependencies for credential encryption:
    - `ring = "0.17"`
    - `base64 = "0.22"`
- Modify `src/config.rs`
  - Add optional `credential_encryption_key: Option<SecretString>`.
  - Add `exposed_credential_encryption_key()` accessor.
  - Add env parsing test coverage.
- Modify `.env`
  - Add a local development `CREDENTIAL_ENCRYPTION_KEY` value.
- Create `migrations/0034_market_feed_admin_config.sql`
  - Create `market_feed_configs`.
  - Create `market_source_credentials`.
- Modify `src/state.rs`
  - Add `market_feed_supervisor: Option<MarketFeedSupervisorHandle>`.
  - Add builder `with_market_feed_supervisor`.
- Modify `src/workers/market_feed.rs`
  - Add `MarketFeedSupervisorHandle`, `MarketFeedRuntimeStatus`, reload/status support, and test seams.
  - Keep old env-driven `run_loop` behavior available for fallback and existing tests.
- Create `src/modules/admin/market_feed_config.rs`
  - Config request/response DTOs.
  - Provider credential DTOs.
  - Credential encrypt/decrypt/mask helpers.
  - DB load/save functions.
  - Input validation helpers that use existing market feed validators indirectly through `MarketFeedRuntimeConfig::new`.
- Modify `src/modules/admin/mod.rs`
  - Export `market_feed_config`.
- Modify `src/modules/admin/routes.rs`
  - Register Admin API endpoints:
    - `GET /market-feed/config`
    - `PATCH /market-feed/config`
    - `POST /market-feed/reload`
    - `GET /market-feed/status`
    - `GET /market-feed/credentials`
    - `PATCH /market-feed/credentials/:provider`
  - Delegate most logic to `admin::market_feed_config` to avoid making `routes.rs` larger.
- Modify `src/main.rs`
  - Create and attach `MarketFeedSupervisorHandle`.
  - Bootstrap supervisor from DB config if present, otherwise fall back to env settings.
- Modify `tests/admin_routes.rs`
  - Add MySQL integration tests for config, credentials, status, and reload endpoint.
- Modify `tests/market_feed_worker.rs`
  - Add pure supervisor tests where possible.

### Frontend

- Create `web/src/admin/actions/MarketFeedConfigPage.tsx`
  - Load config, status, and credentials.
  - Save config.
  - Upsert provider credentials with secret fields not prefilled.
  - Trigger manual reload through `ConfirmAction`.
- Create `web/src/admin/actions/MarketFeedConfigPage.test.tsx`
  - Test render, save request, reload request, and secret masking behavior.
- Modify `web/src/admin/routes.tsx`
  - Add route `market/feed-config`.
- Modify `web/src/layouts/AdminLayout.tsx`
  - Add sidebar child under 行情市场: `行情订阅` -> `/admin/market/feed-config`.
- Modify `web/src/layouts/AdminLayout.test.tsx`
  - Assert the new nav child appears after expanding 行情市场.
- Modify `web/src/shared/StatusTag.tsx` if needed
  - Add `success`, `failed`, `skipped`, `needs_reload` Chinese labels if absent.

### Progress

- Modify `docs/superpowers/PROGRESS.md`
  - Add final implementation entry after verification.

---

## Task 1: Add database schema for Admin market feed config

**Files:**
- Create: `migrations/0034_market_feed_admin_config.sql`
- Test: `tests/admin_routes.rs`

- [ ] **Step 1: Write the failing migration-backed route test**

Append this test to `tests/admin_routes.rs`:

```rust
#[tokio::test]
async fn admin_market_feed_config_tables_are_available_after_migration() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };

    let config_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM market_feed_configs")
        .fetch_one(&pool)
        .await?;
    let credential_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM market_source_credentials")
        .fetch_one(&pool)
        .await?;

    assert_eq!(config_count, 0);
    assert_eq!(credential_count, 0);
    Ok(())
}
```

- [ ] **Step 2: Run RED test**

Run:

```bash
DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_market_feed_config_tables_are_available_after_migration -- --nocapture
```

Expected: FAIL with a MySQL error that `market_feed_configs` or `market_source_credentials` does not exist.

- [ ] **Step 3: Create migration**

Create `migrations/0034_market_feed_admin_config.sql`:

```sql
CREATE TABLE market_feed_configs (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(64) NOT NULL UNIQUE,
    symbols_json JSON NOT NULL,
    intervals_json JSON NOT NULL,
    providers_json JSON NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    version BIGINT UNSIGNED NOT NULL DEFAULT 1,
    applied_version BIGINT UNSIGNED NULL,
    last_reload_status VARCHAR(32) NULL,
    last_reload_error TEXT NULL,
    last_reloaded_at TIMESTAMP(6) NULL,
    updated_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_market_feed_configs_enabled (enabled),
    CONSTRAINT fk_market_feed_configs_updated_by FOREIGN KEY (updated_by) REFERENCES admin_users(id)
);

CREATE TABLE market_source_credentials (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    provider VARCHAR(32) NOT NULL UNIQUE,
    auth_type VARCHAR(32) NOT NULL DEFAULT 'none',
    api_key_ciphertext TEXT NULL,
    api_secret_ciphertext TEXT NULL,
    passphrase_ciphertext TEXT NULL,
    api_key_mask VARCHAR(64) NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    updated_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_market_source_credentials_enabled (enabled),
    CONSTRAINT fk_market_source_credentials_updated_by FOREIGN KEY (updated_by) REFERENCES admin_users(id)
);
```

- [ ] **Step 4: Run GREEN test**

Run:

```bash
DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_market_feed_config_tables_are_available_after_migration -- --nocapture
```

Expected: PASS.

---

## Task 2: Add credential encryption settings and helpers

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/config.rs`
- Modify: `.env`
- Create: `src/modules/admin/market_feed_config.rs`
- Modify: `src/modules/admin/mod.rs`
- Test: `src/modules/admin/market_feed_config.rs`
- Test: `src/config.rs`

- [ ] **Step 1: Add failing config and crypto tests**

In `src/config.rs`, update the existing `settings_from_env_parses_market_feed_lists` test by setting and asserting the key:

```rust
set_test_env("CREDENTIAL_ENCRYPTION_KEY", "0123456789abcdef0123456789abcdef");
```

Add this assertion after settings are loaded:

```rust
assert_eq!(
    settings.exposed_credential_encryption_key(),
    Some("0123456789abcdef0123456789abcdef")
);
```

Also add this cleanup line in `clear_market_feed_env()`:

```rust
env::remove_var("CREDENTIAL_ENCRYPTION_KEY");
```

Create `src/modules/admin/market_feed_config.rs` with only tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_mask_keeps_edges_and_hides_middle() {
        assert_eq!(mask_api_key("abcd1234wxyz"), "abcd****wxyz");
        assert_eq!(mask_api_key("short"), "*****");
    }

    #[test]
    fn credential_encryption_round_trips_without_plaintext_leak() {
        let key = "0123456789abcdef0123456789abcdef";
        let ciphertext = encrypt_credential("secret-value", key).unwrap();

        assert!(!ciphertext.contains("secret-value"));
        assert_eq!(decrypt_credential(&ciphertext, key).unwrap(), "secret-value");
    }

    #[test]
    fn credential_encryption_rejects_short_key() {
        assert!(encrypt_credential("secret", "too-short").is_err());
    }
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib config::tests::settings_from_env_parses_market_feed_lists admin::market_feed_config::tests -- --nocapture
```

Expected: FAIL because `credential_encryption_key`, `exposed_credential_encryption_key`, `mask_api_key`, `encrypt_credential`, and `decrypt_credential` do not exist.

- [ ] **Step 3: Add dependencies**

Modify `Cargo.toml` dependencies:

```toml
base64 = "0.22"
ring = "0.17"
```

- [ ] **Step 4: Add setting**

Modify `src/config.rs`:

```rust
pub credential_encryption_key: Option<SecretString>,
```

Place it after `jwt_secret`.

Add accessor:

```rust
pub fn exposed_credential_encryption_key(&self) -> Option<&str> {
    self.credential_encryption_key
        .as_ref()
        .map(SecretString::expose_secret)
}
```

Update every manual `Settings` construction in tests (`src/lib.rs`, `tests/admin_routes.rs`, and any compiler-reported files) with:

```rust
credential_encryption_key: Some(SecretString::new("0123456789abcdef0123456789abcdef".to_owned())),
```

Modify `.env`:

```env
CREDENTIAL_ENCRYPTION_KEY=0123456789abcdef0123456789abcdef
```

- [ ] **Step 5: Export module**

Modify `src/modules/admin/mod.rs`:

```rust
pub mod market_feed_config;
pub mod routes;
```

- [ ] **Step 6: Implement encryption helpers**

Replace `src/modules/admin/market_feed_config.rs` with:

```rust
use crate::{error::{AppError, AppResult}, modules::market::adapters::MarketFeedProvider};
use base64::{Engine, engine::general_purpose::STANDARD};
use ring::{aead, rand::{SecureRandom, SystemRandom}};

const NONCE_LEN: usize = 12;

pub fn mask_api_key(value: &str) -> String {
    let value = value.trim();
    if value.len() < 8 {
        return "*".repeat(value.len());
    }
    format!("{}****{}", &value[..4], &value[value.len() - 4..])
}

pub fn encrypt_credential(plaintext: &str, key: &str) -> AppResult<String> {
    let key_bytes = key.as_bytes();
    if key_bytes.len() != 32 {
        return Err(AppError::Validation(
            "credential encryption key must be exactly 32 bytes".to_owned(),
        ));
    }
    let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, key_bytes)
        .map_err(|_| AppError::Internal("credential encryption key is invalid".to_owned()))?;
    let key = aead::LessSafeKey::new(unbound_key);
    let rng = SystemRandom::new();
    let mut nonce_bytes = [0_u8; NONCE_LEN];
    rng.fill(&mut nonce_bytes)
        .map_err(|_| AppError::Internal("credential nonce generation failed".to_owned()))?;
    let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);
    let mut in_out = plaintext.as_bytes().to_vec();
    key.seal_in_place_append_tag(nonce, aead::Aad::empty(), &mut in_out)
        .map_err(|_| AppError::Internal("credential encryption failed".to_owned()))?;
    let mut output = nonce_bytes.to_vec();
    output.extend(in_out);
    Ok(STANDARD.encode(output))
}

pub fn decrypt_credential(ciphertext: &str, key: &str) -> AppResult<String> {
    let key_bytes = key.as_bytes();
    if key_bytes.len() != 32 {
        return Err(AppError::Validation(
            "credential encryption key must be exactly 32 bytes".to_owned(),
        ));
    }
    let mut payload = STANDARD
        .decode(ciphertext)
        .map_err(|_| AppError::Validation("credential ciphertext is invalid".to_owned()))?;
    if payload.len() <= NONCE_LEN {
        return Err(AppError::Validation("credential ciphertext is invalid".to_owned()));
    }
    let mut nonce_bytes = [0_u8; NONCE_LEN];
    nonce_bytes.copy_from_slice(&payload[..NONCE_LEN]);
    let mut encrypted = payload.split_off(NONCE_LEN);
    let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, key_bytes)
        .map_err(|_| AppError::Internal("credential encryption key is invalid".to_owned()))?;
    let key = aead::LessSafeKey::new(unbound_key);
    let plaintext = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce_bytes),
            aead::Aad::empty(),
            &mut encrypted,
        )
        .map_err(|_| AppError::Validation("credential ciphertext is invalid".to_owned()))?;
    String::from_utf8(plaintext.to_vec())
        .map_err(|_| AppError::Validation("credential plaintext is invalid utf8".to_owned()))
}

pub fn parse_market_feed_provider(code: &str) -> AppResult<MarketFeedProvider> {
    MarketFeedProvider::from_code(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_mask_keeps_edges_and_hides_middle() {
        assert_eq!(mask_api_key("abcd1234wxyz"), "abcd****wxyz");
        assert_eq!(mask_api_key("short"), "*****");
    }

    #[test]
    fn credential_encryption_round_trips_without_plaintext_leak() {
        let key = "0123456789abcdef0123456789abcdef";
        let ciphertext = encrypt_credential("secret-value", key).unwrap();

        assert!(!ciphertext.contains("secret-value"));
        assert_eq!(decrypt_credential(&ciphertext, key).unwrap(), "secret-value");
    }

    #[test]
    fn credential_encryption_rejects_short_key() {
        assert!(encrypt_credential("secret", "too-short").is_err());
    }
}
```

- [ ] **Step 7: Run GREEN tests**

Run:

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib config::tests::settings_from_env_parses_market_feed_lists admin::market_feed_config::tests -- --nocapture
```

Expected: PASS.

---

## Task 3: Add market feed supervisor handle

**Files:**
- Modify: `src/workers/market_feed.rs`
- Modify: `src/state.rs`
- Test: `tests/market_feed_worker.rs`

- [ ] **Step 1: Write failing supervisor tests**

Append to `tests/market_feed_worker.rs`:

```rust
#[tokio::test]
async fn market_feed_supervisor_status_tracks_reload_success() -> Result<(), Box<dyn Error>> {
    let handle = exchange_api::workers::market_feed::MarketFeedSupervisorHandle::new_for_tests();
    let settings = test_settings();
    let config = exchange_api::workers::market_feed::MarketFeedRuntimeConfig::new(
        &settings,
        vec!["BTC-USDT".to_owned()],
        vec!["1m".to_owned()],
        vec!["htx".to_owned()],
        1,
    )?;

    handle.accept_config_for_tests(config, 7).await?;
    let status = handle.status().await;

    assert_eq!(status.applied_version, Some(7));
    assert_eq!(status.symbols, vec!["BTCUSDT".to_owned()]);
    assert_eq!(status.intervals, vec!["1m".to_owned()]);
    assert_eq!(status.providers, vec!["htx".to_owned()]);
    assert_eq!(status.last_reload_status.as_deref(), Some("success"));
    Ok(())
}

#[tokio::test]
async fn market_feed_supervisor_keeps_old_config_after_failed_acceptance() -> Result<(), Box<dyn Error>> {
    let handle = exchange_api::workers::market_feed::MarketFeedSupervisorHandle::new_for_tests();
    let settings = test_settings();
    let valid = exchange_api::workers::market_feed::MarketFeedRuntimeConfig::new(
        &settings,
        vec!["BTC-USDT".to_owned()],
        vec!["1m".to_owned()],
        vec!["htx".to_owned()],
        1,
    )?;
    handle.accept_config_for_tests(valid, 2).await?;

    let invalid = exchange_api::workers::market_feed::MarketFeedRuntimeConfig::new(
        &settings,
        Vec::new(),
        vec!["1m".to_owned()],
        vec!["htx".to_owned()],
        1,
    )?;
    assert!(handle.accept_config_for_tests(invalid, 3).await.is_err());

    let status = handle.status().await;
    assert_eq!(status.applied_version, Some(2));
    assert_eq!(status.symbols, vec!["BTCUSDT".to_owned()]);
    assert_eq!(status.last_reload_status.as_deref(), Some("failed"));
    Ok(())
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_feed_worker market_feed_supervisor -- --nocapture
```

Expected: FAIL because `MarketFeedSupervisorHandle` does not exist.

- [ ] **Step 3: Implement handle types**

Add to `src/workers/market_feed.rs` after `MarketFeedRuntimeConfig`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct MarketFeedRuntimeStatus {
    pub applied_version: Option<u64>,
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub last_reload_status: Option<String>,
    pub last_reload_error: Option<String>,
}

#[derive(Debug, Default)]
struct MarketFeedSupervisorState {
    status: MarketFeedRuntimeStatus,
    task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Clone, Debug, Default)]
pub struct MarketFeedSupervisorHandle {
    inner: std::sync::Arc<tokio::sync::Mutex<MarketFeedSupervisorState>>,
}

impl MarketFeedSupervisorHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_for_tests() -> Self {
        Self::new()
    }

    pub async fn status(&self) -> MarketFeedRuntimeStatus {
        self.inner.lock().await.status.clone()
    }

    pub async fn reload(&self, state: AppState, config: MarketFeedRuntimeConfig, version: u64) -> AppResult<()> {
        if !config.enabled() {
            return self.stop_with_status(version, "skipped", None).await;
        }
        let symbol_refs: Vec<&str> = config.symbols().iter().map(String::as_str).collect();
        let interval_refs: Vec<&str> = config.intervals().iter().map(String::as_str).collect();
        MarketFeedWorker::<MarketIngestionService>::provider_configs_for(
            &state.settings,
            config.providers(),
            &symbol_refs,
            &interval_refs,
        )?;
        let next_status = status_from_config(&config, version, "success", None);
        let mut guard = self.inner.lock().await;
        if let Some(task) = guard.task.take() {
            task.abort();
        }
        let task_config = config.clone();
        guard.task = Some(tokio::spawn(async move {
            if let Err(error) = run_config_loop(state, task_config).await {
                tracing::error!(%error, "market feed supervised loop stopped");
            }
        }));
        guard.status = next_status;
        Ok(())
    }

    pub async fn accept_config_for_tests(&self, config: MarketFeedRuntimeConfig, version: u64) -> AppResult<()> {
        if !config.enabled() {
            let error = "market feed symbols are required".to_owned();
            self.inner.lock().await.status.last_reload_status = Some("failed".to_owned());
            self.inner.lock().await.status.last_reload_error = Some(error.clone());
            return Err(AppError::Validation(error));
        }
        self.inner.lock().await.status = status_from_config(&config, version, "success", None);
        Ok(())
    }

    async fn stop_with_status(&self, version: u64, status: &str, error: Option<String>) -> AppResult<()> {
        let mut guard = self.inner.lock().await;
        if let Some(task) = guard.task.take() {
            task.abort();
        }
        guard.status = MarketFeedRuntimeStatus {
            applied_version: Some(version),
            symbols: Vec::new(),
            intervals: Vec::new(),
            providers: Vec::new(),
            last_reload_status: Some(status.to_owned()),
            last_reload_error: error,
        };
        Ok(())
    }
}

fn status_from_config(
    config: &MarketFeedRuntimeConfig,
    version: u64,
    status: &str,
    error: Option<String>,
) -> MarketFeedRuntimeStatus {
    MarketFeedRuntimeStatus {
        applied_version: Some(version),
        symbols: config.symbols().to_vec(),
        intervals: config.intervals().to_vec(),
        providers: config.providers().iter().map(|provider| provider.code().to_owned()).collect(),
        last_reload_status: Some(status.to_owned()),
        last_reload_error: error,
    }
}
```

- [ ] **Step 4: Add supervisor to AppState**

Modify `src/state.rs`:

```rust
use crate::{config::Settings, modules::events::EventBroadcastHub, workers::market_feed::MarketFeedSupervisorHandle};
```

Add field:

```rust
pub market_feed_supervisor: Option<MarketFeedSupervisorHandle>,
```

Initialize with `None` in `AppState::new`.

Add builder:

```rust
pub fn with_market_feed_supervisor(mut self, supervisor: MarketFeedSupervisorHandle) -> Self {
    self.market_feed_supervisor = Some(supervisor);
    self
}
```

- [ ] **Step 5: Run GREEN tests**

Run:

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_feed_worker market_feed_supervisor -- --nocapture
```

Expected: PASS.

---

## Task 4: Add Admin market feed config and credential APIs

**Files:**
- Modify: `src/modules/admin/market_feed_config.rs`
- Modify: `src/modules/admin/routes.rs`
- Test: `tests/admin_routes.rs`

- [ ] **Step 1: Add failing Admin API tests**

Append to `tests/admin_routes.rs`:

```rust
#[tokio::test]
async fn admin_can_save_and_read_market_feed_config() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let token = admin_token("1001");
    let app = build_router(AppState::new(test_settings()).with_mysql(pool));

    let save = app.clone().oneshot(
        Request::builder()
            .method("PATCH")
            .uri("/admin/api/v1/market-feed/config")
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "enabled": true,
                "symbols": ["BTC-USDT", "ETH-USDT"],
                "intervals": ["1m", "5m"],
                "providers": ["bitget", "htx"],
                "reason": "enable public market feed"
            }).to_string()))?,
    ).await?;
    assert_eq!(save.status(), StatusCode::OK);
    let save_body: Value = serde_json::from_slice(&to_bytes(save.into_body(), 1024 * 1024).await?)?;
    assert_eq!(save_body["symbols"], json!(["BTCUSDT", "ETHUSDT"]));
    assert_eq!(save_body["needs_reload"], json!(true));

    let read = app.oneshot(
        Request::builder()
            .uri("/admin/api/v1/market-feed/config")
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())?,
    ).await?;
    assert_eq!(read.status(), StatusCode::OK);
    let read_body: Value = serde_json::from_slice(&to_bytes(read.into_body(), 1024 * 1024).await?)?;
    assert_eq!(read_body["providers"], json!(["bitget", "htx"]));
    assert_eq!(read_body["version"], save_body["version"]);
    Ok(())
}

#[tokio::test]
async fn admin_market_feed_config_rejects_invalid_interval() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let token = admin_token("1002");
    let app = build_router(AppState::new(test_settings()).with_mysql(pool));

    let response = app.oneshot(
        Request::builder()
            .method("PATCH")
            .uri("/admin/api/v1/market-feed/config")
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "enabled": true,
                "symbols": ["BTC-USDT"],
                "intervals": ["2m"],
                "providers": ["bitget"],
                "reason": "bad interval"
            }).to_string()))?,
    ).await?;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn admin_can_upsert_market_feed_credential_without_secret_leak() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let token = admin_token("1003");
    let app = build_router(AppState::new(test_settings()).with_mysql(pool));

    let response = app.clone().oneshot(
        Request::builder()
            .method("PATCH")
            .uri("/admin/api/v1/market-feed/credentials/bitget")
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "auth_type": "api_key",
                "api_key": "abcd1234wxyz",
                "api_secret": "super-secret",
                "passphrase": "trade-passphrase",
                "enabled": true,
                "reason": "configure bitget key"
            }).to_string()))?,
    ).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body_text = String::from_utf8(to_bytes(response.into_body(), 1024 * 1024).await?.to_vec())?;
    assert!(body_text.contains("abcd****wxyz"));
    assert!(!body_text.contains("super-secret"));
    assert!(!body_text.contains("trade-passphrase"));

    let list = app.oneshot(
        Request::builder()
            .uri("/admin/api/v1/market-feed/credentials")
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())?,
    ).await?;
    let list_text = String::from_utf8(to_bytes(list.into_body(), 1024 * 1024).await?.to_vec())?;
    assert!(list_text.contains("abcd****wxyz"));
    assert!(!list_text.contains("super-secret"));
    Ok(())
}
```

If `admin_token` helper does not exist, add this helper near other test helpers:

```rust
fn admin_token(subject: &str) -> String {
    issue_token(
        subject,
        TokenScope::Admin,
        "test-secret",
        chrono::Duration::minutes(30),
    )
    .unwrap()
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_market_feed -- --nocapture
```

Expected: FAIL with 404 for market-feed routes or missing implementation.

- [ ] **Step 3: Implement DTOs and repository functions**

Extend `src/modules/admin/market_feed_config.rs` with these public types and functions:

```rust
use crate::{
    config::Settings,
    error::{AppError, AppResult},
    state::AppState,
    time::{option_unix_millis, unix_millis},
    workers::market_feed::MarketFeedRuntimeConfig,
};
use chrono::{DateTime, Utc};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{MySql, Pool, types::Json as SqlxJson};

#[derive(Debug, Deserialize)]
pub struct SaveMarketFeedConfigRequest {
    pub enabled: bool,
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MarketFeedConfigResponse {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    pub version: u64,
    pub applied_version: Option<u64>,
    pub last_reload_status: Option<String>,
    pub last_reload_error: Option<String>,
    #[serde(with = "option_unix_millis")]
    pub last_reloaded_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub updated_at: DateTime<Utc>,
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub needs_reload: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct MarketFeedConfigRow {
    id: u64,
    name: String,
    symbols_json: SqlxJson<Vec<String>>,
    intervals_json: SqlxJson<Vec<String>>,
    providers_json: SqlxJson<Vec<String>>,
    enabled: bool,
    version: u64,
    applied_version: Option<u64>,
    last_reload_status: Option<String>,
    last_reload_error: Option<String>,
    last_reloaded_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertMarketSourceCredentialRequest {
    pub auth_type: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
    pub enabled: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MarketSourceCredentialResponse {
    pub provider: String,
    pub auth_type: String,
    pub api_key_mask: Option<String>,
    pub enabled: bool,
    #[serde(with = "unix_millis")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct MarketSourceCredentialsResponse {
    pub credentials: Vec<MarketSourceCredentialResponse>,
}

pub async fn load_config(pool: &Pool<MySql>) -> AppResult<Option<MarketFeedConfigResponse>> {
    let row = sqlx::query_as::<_, MarketFeedConfigRow>(
        r#"SELECT id, name, symbols_json, intervals_json, providers_json, enabled, version,
                  applied_version, last_reload_status, last_reload_error, last_reloaded_at, updated_at
           FROM market_feed_configs
           WHERE name = 'default'"#,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(config_response_from_row))
}

pub async fn save_config(
    pool: &Pool<MySql>,
    settings: &Settings,
    admin_id: u64,
    request: SaveMarketFeedConfigRequest,
) -> AppResult<MarketFeedConfigResponse> {
    let validated = validate_config_request(settings, &request)?;
    sqlx::query(
        r#"INSERT INTO market_feed_configs
           (name, symbols_json, intervals_json, providers_json, enabled, version, updated_by)
           VALUES ('default', ?, ?, ?, ?, 1, ?)
           ON DUPLICATE KEY UPDATE
             symbols_json = VALUES(symbols_json),
             intervals_json = VALUES(intervals_json),
             providers_json = VALUES(providers_json),
             enabled = VALUES(enabled),
             version = version + 1,
             updated_by = VALUES(updated_by)"#,
    )
    .bind(SqlxJson(validated.symbols))
    .bind(SqlxJson(validated.intervals))
    .bind(SqlxJson(validated.providers))
    .bind(request.enabled)
    .bind(admin_id)
    .execute(pool)
    .await?;
    load_config(pool).await?.ok_or(AppError::NotFound)
}

pub async fn list_credentials(pool: &Pool<MySql>) -> AppResult<MarketSourceCredentialsResponse> {
    let credentials = sqlx::query_as::<_, MarketSourceCredentialResponse>(
        r#"SELECT provider, auth_type, api_key_mask, enabled, updated_at
           FROM market_source_credentials
           ORDER BY provider ASC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(MarketSourceCredentialsResponse { credentials })
}

pub async fn upsert_credential(
    pool: &Pool<MySql>,
    settings: &Settings,
    admin_id: u64,
    provider: &str,
    request: UpsertMarketSourceCredentialRequest,
) -> AppResult<MarketSourceCredentialResponse> {
    parse_market_feed_provider(provider)?;
    if request.auth_type != "none" && request.auth_type != "api_key" {
        return Err(AppError::Validation("invalid market source auth type".to_owned()));
    }
    let key = settings
        .exposed_credential_encryption_key()
        .ok_or_else(|| AppError::Internal("credential encryption key is not configured".to_owned()))?;
    let api_key_ciphertext = request.api_key.as_deref().map(|value| encrypt_credential(value, key)).transpose()?;
    let api_secret_ciphertext = request.api_secret.as_deref().map(|value| encrypt_credential(value, key)).transpose()?;
    let passphrase_ciphertext = request.passphrase.as_deref().map(|value| encrypt_credential(value, key)).transpose()?;
    let api_key_mask = request.api_key.as_deref().map(mask_api_key);

    sqlx::query(
        r#"INSERT INTO market_source_credentials
           (provider, auth_type, api_key_ciphertext, api_secret_ciphertext, passphrase_ciphertext, api_key_mask, enabled, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
             auth_type = VALUES(auth_type),
             api_key_ciphertext = COALESCE(VALUES(api_key_ciphertext), api_key_ciphertext),
             api_secret_ciphertext = COALESCE(VALUES(api_secret_ciphertext), api_secret_ciphertext),
             passphrase_ciphertext = COALESCE(VALUES(passphrase_ciphertext), passphrase_ciphertext),
             api_key_mask = COALESCE(VALUES(api_key_mask), api_key_mask),
             enabled = VALUES(enabled),
             updated_by = VALUES(updated_by)"#,
    )
    .bind(provider)
    .bind(request.auth_type.trim())
    .bind(api_key_ciphertext)
    .bind(api_secret_ciphertext)
    .bind(passphrase_ciphertext)
    .bind(api_key_mask)
    .bind(request.enabled)
    .bind(admin_id)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, MarketSourceCredentialResponse>(
        r#"SELECT provider, auth_type, api_key_mask, enabled, updated_at
           FROM market_source_credentials WHERE provider = ?"#,
    )
    .bind(provider)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

struct ValidatedConfig {
    symbols: Vec<String>,
    intervals: Vec<String>,
    providers: Vec<String>,
}

fn validate_config_request(settings: &Settings, request: &SaveMarketFeedConfigRequest) -> AppResult<ValidatedConfig> {
    if !request.enabled {
        return Ok(ValidatedConfig {
            symbols: Vec::new(),
            intervals: request.intervals.clone(),
            providers: request.providers.clone(),
        });
    }
    let config = MarketFeedRuntimeConfig::new(
        settings,
        request.symbols.clone(),
        request.intervals.clone(),
        request.providers.clone(),
        settings.market_feed_reconnect_seconds,
    )?;
    Ok(ValidatedConfig {
        symbols: config.symbols().to_vec(),
        intervals: config.intervals().to_vec(),
        providers: config.providers().iter().map(|provider| provider.code().to_owned()).collect(),
    })
}

fn config_response_from_row(row: MarketFeedConfigRow) -> MarketFeedConfigResponse {
    let symbols = row.symbols_json.0;
    let intervals = row.intervals_json.0;
    let providers = row.providers_json.0;
    let needs_reload = row.applied_version != Some(row.version);
    MarketFeedConfigResponse {
        id: row.id,
        name: row.name,
        enabled: row.enabled,
        version: row.version,
        applied_version: row.applied_version,
        last_reload_status: row.last_reload_status,
        last_reload_error: row.last_reload_error,
        last_reloaded_at: row.last_reloaded_at,
        updated_at: row.updated_at,
        symbols,
        intervals,
        providers,
        needs_reload,
    }
}
```

Keep the encryption helper tests from Task 2 at the bottom of this file.

- [ ] **Step 4: Register routes**

Modify `src/modules/admin/routes.rs` imports:

```rust
use super::market_feed_config::{
    MarketSourceCredentialsResponse, SaveMarketFeedConfigRequest,
    UpsertMarketSourceCredentialRequest,
};
```

Add route registrations in `routes()`:

```rust
.route("/market-feed/config", get(get_market_feed_config).patch(save_market_feed_config))
.route("/market-feed/credentials", get(list_market_feed_credentials))
.route("/market-feed/credentials/:provider", patch(upsert_market_feed_credential))
```

Add handlers:

```rust
async fn get_market_feed_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Value>> {
    let pool = mysql_pool(&state)?;
    let config = super::market_feed_config::load_config(&pool).await?;
    Ok(Json(json!({ "config": config })))
}

async fn save_market_feed_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveMarketFeedConfigRequest>,
) -> AppResult<Json<super::market_feed_config::MarketFeedConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let reason = request.reason.clone();
    let config = super::market_feed_config::save_config(&pool, &state.settings, admin_id, request).await?;
    insert_typed_admin_audit_log(
        &pool,
        admin_id,
        AdminAuditEntry {
            action: "market_feed.config.save",
            target_type: "market_feed_config",
            target_id: config.id,
            before_json: None,
            after_json: Some(json!({
                "symbols": config.symbols,
                "intervals": config.intervals,
                "providers": config.providers,
                "enabled": config.enabled,
                "version": config.version
            })),
            reason,
        },
    ).await?;
    Ok(Json(config))
}

async fn list_market_feed_credentials(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarketSourceCredentialsResponse>> {
    let pool = mysql_pool(&state)?;
    Ok(Json(super::market_feed_config::list_credentials(&pool).await?))
}

async fn upsert_market_feed_credential(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Json(request): Json<UpsertMarketSourceCredentialRequest>,
) -> AppResult<Json<super::market_feed_config::MarketSourceCredentialResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let reason = request.reason.clone();
    let credential = super::market_feed_config::upsert_credential(&pool, &state.settings, admin_id, &provider, request).await?;
    insert_typed_admin_audit_log(
        &pool,
        admin_id,
        AdminAuditEntry {
            action: "market_feed.credential.upsert",
            target_type: "market_source_credential",
            target_id: 0,
            before_json: None,
            after_json: Some(json!({
                "provider": credential.provider,
                "auth_type": credential.auth_type,
                "api_key_mask": credential.api_key_mask,
                "enabled": credential.enabled
            })),
            reason,
        },
    ).await?;
    Ok(Json(credential))
}
```

If `insert_typed_admin_audit_log` does not exist for pool-level use, use a transaction or add a small wrapper around the existing in-transaction helper.

- [ ] **Step 5: Run GREEN tests**

Run:

```bash
DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_market_feed -- --nocapture
```

Expected: PASS.

---

## Task 5: Add manual reload and status APIs

**Files:**
- Modify: `src/modules/admin/market_feed_config.rs`
- Modify: `src/modules/admin/routes.rs`
- Modify: `src/main.rs`
- Test: `tests/admin_routes.rs`

- [ ] **Step 1: Add failing reload/status tests**

Append to `tests/admin_routes.rs`:

```rust
#[tokio::test]
async fn admin_can_reload_market_feed_config_and_read_status() -> Result<(), Box<dyn Error>> {
    let Some(pool) = mysql_pool().await else {
        return Ok(());
    };
    let token = admin_token("1004");
    let supervisor = exchange_api::workers::market_feed::MarketFeedSupervisorHandle::new_for_tests();
    let state = AppState::new(test_settings())
        .with_mysql(pool)
        .with_market_feed_supervisor(supervisor);
    let app = build_router(state);

    let save = app.clone().oneshot(
        Request::builder()
            .method("PATCH")
            .uri("/admin/api/v1/market-feed/config")
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "enabled": true,
                "symbols": ["BTC-USDT"],
                "intervals": ["1m"],
                "providers": ["htx"],
                "reason": "prepare reload"
            }).to_string()))?,
    ).await?;
    assert_eq!(save.status(), StatusCode::OK);

    let reload = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/admin/api/v1/market-feed/reload")
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(json!({ "reason": "apply market feed config" }).to_string()))?,
    ).await?;
    assert_eq!(reload.status(), StatusCode::OK);
    let reload_body: Value = serde_json::from_slice(&to_bytes(reload.into_body(), 1024 * 1024).await?)?;
    assert_eq!(reload_body["last_reload_status"], json!("success"));
    assert_eq!(reload_body["symbols"], json!(["BTCUSDT"]));

    let status = app.oneshot(
        Request::builder()
            .uri("/admin/api/v1/market-feed/status")
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())?,
    ).await?;
    assert_eq!(status.status(), StatusCode::OK);
    let status_body: Value = serde_json::from_slice(&to_bytes(status.into_body(), 1024 * 1024).await?)?;
    assert_eq!(status_body["applied_version"], reload_body["applied_version"]);
    Ok(())
}
```

- [ ] **Step 2: Run RED test**

Run:

```bash
DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_can_reload_market_feed_config_and_read_status -- --nocapture
```

Expected: FAIL with 404 for reload/status routes or missing implementation.

- [ ] **Step 3: Add reload request and DB status update**

Extend `src/modules/admin/market_feed_config.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct ReloadMarketFeedRequest {
    pub reason: Option<String>,
}

pub async fn mark_reload_success(pool: &Pool<MySql>, version: u64) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE market_feed_configs
           SET applied_version = ?, last_reload_status = 'success', last_reload_error = NULL, last_reloaded_at = CURRENT_TIMESTAMP(6)
           WHERE name = 'default'"#,
    )
    .bind(version)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_reload_failed(pool: &Pool<MySql>, error: &str) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE market_feed_configs
           SET last_reload_status = 'failed', last_reload_error = ?, last_reloaded_at = CURRENT_TIMESTAMP(6)
           WHERE name = 'default'"#,
    )
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 4: Register reload/status routes**

Modify `src/modules/admin/routes.rs` route registrations:

```rust
.route("/market-feed/reload", post(reload_market_feed_config))
.route("/market-feed/status", get(get_market_feed_status))
```

Add imports:

```rust
ReloadMarketFeedRequest,
```

Add handlers:

```rust
async fn reload_market_feed_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<ReloadMarketFeedRequest>,
) -> AppResult<Json<crate::workers::market_feed::MarketFeedRuntimeStatus>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let pool = mysql_pool(&state)?;
    let supervisor = state
        .market_feed_supervisor
        .clone()
        .ok_or_else(|| AppError::Internal("market feed supervisor is not configured".to_owned()))?;
    let config = super::market_feed_config::load_config(&pool).await?.ok_or(AppError::NotFound)?;
    let runtime_config = crate::workers::market_feed::MarketFeedRuntimeConfig::new(
        &state.settings,
        config.symbols.clone(),
        config.intervals.clone(),
        config.providers.clone(),
        state.settings.market_feed_reconnect_seconds,
    )?;

    match supervisor.reload(state.clone(), runtime_config, config.version).await {
        Ok(()) => {
            super::market_feed_config::mark_reload_success(&pool, config.version).await?;
        }
        Err(error) => {
            let message = error.to_string();
            super::market_feed_config::mark_reload_failed(&pool, &message).await?;
            return Err(error);
        }
    }
    let status = supervisor.status().await;
    insert_typed_admin_audit_log(
        &pool,
        admin_id,
        AdminAuditEntry {
            action: "market_feed.reload",
            target_type: "market_feed_config",
            target_id: config.id,
            before_json: None,
            after_json: Some(json!({
                "applied_version": status.applied_version,
                "symbols": status.symbols,
                "intervals": status.intervals,
                "providers": status.providers,
                "last_reload_status": status.last_reload_status
            })),
            reason: request.reason,
        },
    ).await?;
    Ok(Json(status))
}

async fn get_market_feed_status(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<crate::workers::market_feed::MarketFeedRuntimeStatus>> {
    let supervisor = state
        .market_feed_supervisor
        .clone()
        .ok_or_else(|| AppError::Internal("market feed supervisor is not configured".to_owned()))?;
    Ok(Json(supervisor.status().await))
}
```

- [ ] **Step 5: Attach supervisor in main**

Modify `src/main.rs` before `AppState::new(settings)`:

```rust
let market_feed_supervisor = market_feed::MarketFeedSupervisorHandle::new();
```

Attach it:

```rust
.with_market_feed_supervisor(market_feed_supervisor.clone())
```

Replace the old direct `tokio::spawn` market feed startup with:

```rust
let market_feed_state = state.clone();
tokio::spawn(async move {
    let config = market_feed::MarketFeedRuntimeConfig::new(
        &market_feed_state.settings,
        market_feed_state.settings.market_feed_symbols.clone(),
        market_feed_state.settings.market_feed_intervals.clone(),
        market_feed_state.settings.market_feed_providers.clone(),
        market_feed_state.settings.market_feed_reconnect_seconds,
    );
    match config {
        Ok(config) if config.enabled() => {
            if let Some(supervisor) = market_feed_state.market_feed_supervisor.clone() {
                if let Err(error) = supervisor.reload(market_feed_state, config, 0).await {
                    tracing::error!(%error, "market feed bootstrap reload failed");
                }
            }
        }
        Ok(_) => tracing::info!("market feed websocket loop disabled because no symbols are configured"),
        Err(error) => tracing::error!(%error, "market feed bootstrap config failed"),
    }
});
```

- [ ] **Step 6: Run GREEN test**

Run:

```bash
DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_can_reload_market_feed_config_and_read_status -- --nocapture
```

Expected: PASS.

---

## Task 6: Add Admin frontend market feed configuration page

**Files:**
- Create: `web/src/admin/actions/MarketFeedConfigPage.tsx`
- Create: `web/src/admin/actions/MarketFeedConfigPage.test.tsx`
- Modify: `web/src/admin/routes.tsx`
- Modify: `web/src/layouts/AdminLayout.tsx`
- Modify: `web/src/layouts/AdminLayout.test.tsx`

- [ ] **Step 1: Write failing frontend tests**

Create `web/src/admin/actions/MarketFeedConfigPage.test.tsx`:

```tsx
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { MarketFeedConfigPage } from './MarketFeedConfigPage';

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  global.fetch = fetchMock;
  localStorage.setItem(
    'rust-chain-admin-session',
    JSON.stringify({ accessToken: 'token', refreshToken: 'refresh', scope: 'admin', subject: 'root-admin' })
  );
});

describe('MarketFeedConfigPage', () => {
  it('renders config, status, and masked credentials without secrets', async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({
        config: {
          enabled: true,
          symbols: ['BTCUSDT'],
          intervals: ['1m'],
          providers: ['bitget'],
          version: 3,
          applied_version: 2,
          needs_reload: true,
          last_reload_status: 'success'
        }
      }))
      .mockResolvedValueOnce(jsonResponse({ applied_version: 2, symbols: ['BTCUSDT'], intervals: ['1m'], providers: ['bitget'], last_reload_status: 'success' }))
      .mockResolvedValueOnce(jsonResponse({ credentials: [{ provider: 'bitget', auth_type: 'api_key', api_key_mask: 'abcd****wxyz', enabled: true, updated_at: 1710000000000 }] }));

    render(<MarketFeedConfigPage />);

    expect(await screen.findByDisplayValue('BTCUSDT')).toBeInTheDocument();
    expect(screen.getByText('需要重载')).toBeInTheDocument();
    expect(screen.getByText('abcd****wxyz')).toBeInTheDocument();
    expect(screen.queryByText('super-secret')).not.toBeInTheDocument();
  });

  it('saves config and reloads with reason', async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ config: null }))
      .mockResolvedValueOnce(jsonResponse({ applied_version: null, symbols: [], intervals: [], providers: [], last_reload_status: null }))
      .mockResolvedValueOnce(jsonResponse({ credentials: [] }))
      .mockResolvedValueOnce(jsonResponse({ enabled: true, symbols: ['BTCUSDT'], intervals: ['1m'], providers: ['htx'], version: 1, needs_reload: true }))
      .mockResolvedValueOnce(jsonResponse({ applied_version: 1, symbols: ['BTCUSDT'], intervals: ['1m'], providers: ['htx'], last_reload_status: 'success' }));

    render(<MarketFeedConfigPage />);

    fireEvent.change(await screen.findByLabelText('订阅交易对'), { target: { value: 'BTC-USDT' } });
    fireEvent.change(screen.getByLabelText('K线周期'), { target: { value: '1m' } });
    fireEvent.change(screen.getByLabelText('行情源'), { target: { value: 'htx' } });
    fireEvent.change(screen.getByLabelText('保存原因'), { target: { value: 'save feed config' } });
    fireEvent.click(screen.getByRole('button', { name: '保存配置' }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith('/admin/api/v1/market-feed/config', expect.objectContaining({ method: 'PATCH' })));

    fireEvent.click(screen.getByRole('button', { name: '重载行情订阅' }));
    fireEvent.change(await screen.findByLabelText('操作原因'), { target: { value: 'apply feed config' } });
    fireEvent.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith('/admin/api/v1/market-feed/reload', expect.objectContaining({ method: 'POST' })));
  });
});

function jsonResponse(body: unknown) {
  return Promise.resolve({
    ok: true,
    status: 200,
    json: () => Promise.resolve(body)
  } as Response);
}
```

Update `web/src/admin/routes.test.tsx`:

```tsx
it('wires market feed config to a dedicated action page', () => {
  expect(routeElementName('market/feed-config')).toBe('MarketFeedConfigPage');
});
```

Update `web/src/layouts/AdminLayout.test.tsx` expected 行情市场 children to include `行情订阅`.

- [ ] **Step 2: Run RED frontend tests**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/admin/actions/MarketFeedConfigPage.test.tsx src/admin/routes.test.tsx src/layouts/AdminLayout.test.tsx
```

Expected: FAIL because `MarketFeedConfigPage` and route/nav entries do not exist.

- [ ] **Step 3: Implement page**

Create `web/src/admin/actions/MarketFeedConfigPage.tsx`:

```tsx
import { Button, Card, Space, Switch, TextArea, Toast, Typography } from '@douyinfe/semi-ui';
import { useEffect, useState } from 'react';

import { ApiError, apiRequest } from '../../api/client';
import { PageHeader } from '../../layouts/PageHeader';
import { ConfirmAction } from '../../shared/ConfirmAction';

const { Text, Title } = Typography;

type ConfigResponse = {
  config: null | {
    enabled: boolean;
    symbols: string[];
    intervals: string[];
    providers: string[];
    version: number;
    applied_version?: number | null;
    needs_reload: boolean;
    last_reload_status?: string | null;
    last_reload_error?: string | null;
  };
};

type RuntimeStatus = {
  applied_version?: number | null;
  symbols: string[];
  intervals: string[];
  providers: string[];
  last_reload_status?: string | null;
  last_reload_error?: string | null;
};

type Credential = {
  provider: string;
  auth_type: string;
  api_key_mask?: string | null;
  enabled: boolean;
  updated_at: number;
};

type CredentialsResponse = { credentials: Credential[] };

const emptyStatus: RuntimeStatus = { applied_version: null, symbols: [], intervals: [], providers: [], last_reload_status: null };

function splitCsv(value: string) {
  return value.split(',').map((item) => item.trim()).filter(Boolean);
}

function errorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '操作失败';
}

export function MarketFeedConfigPage() {
  const [enabled, setEnabled] = useState(true);
  const [symbols, setSymbols] = useState('');
  const [intervals, setIntervals] = useState('1m,5m,15m,1h,1d');
  const [providers, setProviders] = useState('bitget,htx');
  const [reason, setReason] = useState('');
  const [needsReload, setNeedsReload] = useState(false);
  const [version, setVersion] = useState<number | null>(null);
  const [status, setStatus] = useState<RuntimeStatus>(emptyStatus);
  const [credentials, setCredentials] = useState<Credential[]>([]);
  const [credentialProvider, setCredentialProvider] = useState('bitget');
  const [credentialApiKey, setCredentialApiKey] = useState('');
  const [credentialSecret, setCredentialSecret] = useState('');
  const [credentialPassphrase, setCredentialPassphrase] = useState('');
  const [credentialReason, setCredentialReason] = useState('');

  async function load() {
    const [configPayload, runtimeStatus, credentialPayload] = await Promise.all([
      apiRequest<ConfigResponse>('/admin/api/v1/market-feed/config'),
      apiRequest<RuntimeStatus>('/admin/api/v1/market-feed/status'),
      apiRequest<CredentialsResponse>('/admin/api/v1/market-feed/credentials')
    ]);
    if (configPayload.config) {
      setEnabled(configPayload.config.enabled);
      setSymbols(configPayload.config.symbols.join(','));
      setIntervals(configPayload.config.intervals.join(','));
      setProviders(configPayload.config.providers.join(','));
      setNeedsReload(configPayload.config.needs_reload);
      setVersion(configPayload.config.version);
    }
    setStatus(runtimeStatus);
    setCredentials(credentialPayload.credentials);
  }

  useEffect(() => {
    load().catch((error) => Toast.error(errorMessage(error)));
  }, []);

  async function saveConfig() {
    try {
      const saved = await apiRequest<{ version: number; needs_reload: boolean }>('/admin/api/v1/market-feed/config', {
        method: 'PATCH',
        body: JSON.stringify({
          enabled,
          symbols: splitCsv(symbols),
          intervals: splitCsv(intervals),
          providers: splitCsv(providers),
          reason
        })
      });
      setVersion(saved.version);
      setNeedsReload(saved.needs_reload);
      Toast.success('行情订阅配置已保存');
    } catch (error) {
      Toast.error(errorMessage(error));
      throw error;
    }
  }

  async function reloadConfig(reloadReason: string) {
    try {
      const nextStatus = await apiRequest<RuntimeStatus>('/admin/api/v1/market-feed/reload', {
        method: 'POST',
        body: JSON.stringify({ reason: reloadReason })
      });
      setStatus(nextStatus);
      setNeedsReload(false);
      Toast.success('行情订阅已重载');
    } catch (error) {
      Toast.error(errorMessage(error));
      throw error;
    }
  }

  async function saveCredential() {
    try {
      await apiRequest(`/admin/api/v1/market-feed/credentials/${credentialProvider}`, {
        method: 'PATCH',
        body: JSON.stringify({
          auth_type: 'api_key',
          api_key: credentialApiKey || undefined,
          api_secret: credentialSecret || undefined,
          passphrase: credentialPassphrase || undefined,
          enabled: true,
          reason: credentialReason
        })
      });
      setCredentialApiKey('');
      setCredentialSecret('');
      setCredentialPassphrase('');
      setCredentialReason('');
      await load();
      Toast.success('行情源凭证已保存');
    } catch (error) {
      Toast.error(errorMessage(error));
      throw error;
    }
  }

  return (
    <main className="exchange-page admin-action-page">
      <PageHeader title="行情订阅配置" description="配置第三方行情订阅、行情源凭证，并由管理员手动重载运行中的订阅。" />
      <div className="admin-action-grid">
        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>订阅配置</Title>
            <Text type="secondary">保存配置不会立即影响当前订阅；点击重载后才会应用。</Text>
            <label className="admin-action-checkbox"><Switch checked={enabled} onChange={setEnabled} /> 启用第三方行情订阅</label>
            <div className="admin-action-form admin-action-form-narrow">
              <label>订阅交易对<TextArea aria-label="订阅交易对" value={symbols} onChange={setSymbols} placeholder="BTC-USDT,ETH-USDT" /></label>
              <label>K线周期<TextArea aria-label="K线周期" value={intervals} onChange={setIntervals} placeholder="1m,5m,15m,1h,1d" /></label>
              <label>行情源<TextArea aria-label="行情源" value={providers} onChange={setProviders} placeholder="bitget,htx" /></label>
              <label>保存原因<TextArea aria-label="保存原因" value={reason} onChange={setReason} placeholder="说明本次修改原因" /></label>
            </div>
            <Space>
              <Button onClick={saveConfig} theme="solid" type="primary">保存配置</Button>
              <ConfirmAction actionText="重载行情订阅" title="确认重载行情订阅" onConfirm={reloadConfig} />
            </Space>
            <Text>保存版本：{version ?? '-'}</Text>
            <Text>应用版本：{status.applied_version ?? '-'}</Text>
            {needsReload ? <Text type="warning">需要重载</Text> : <Text type="success">配置已应用</Text>}
            <Text>最近状态：{status.last_reload_status ?? '-'}</Text>
            {status.last_reload_error ? <Text type="danger">{status.last_reload_error}</Text> : null}
          </Space>
        </Card>

        <Card bordered={false} shadows="always">
          <Space align="start" spacing={16} vertical style={{ width: '100%' }}>
            <Title heading={4}>行情源凭证</Title>
            <Text type="secondary">凭证保存后只显示 API Key 掩码，Secret 不会回显。</Text>
            <div className="admin-action-form admin-action-form-narrow">
              <label>Provider<select value={credentialProvider} onChange={(event) => setCredentialProvider(event.currentTarget.value)}><option value="bitget">bitget</option><option value="htx">htx</option></select></label>
              <label>API Key<input value={credentialApiKey} onChange={(event) => setCredentialApiKey(event.currentTarget.value)} /></label>
              <label>API Secret<input type="password" value={credentialSecret} onChange={(event) => setCredentialSecret(event.currentTarget.value)} /></label>
              <label>Passphrase<input type="password" value={credentialPassphrase} onChange={(event) => setCredentialPassphrase(event.currentTarget.value)} /></label>
              <label>保存原因<TextArea value={credentialReason} onChange={setCredentialReason} placeholder="说明凭证修改原因" /></label>
            </div>
            <Button onClick={saveCredential} theme="solid" type="primary">保存凭证</Button>
            <div>
              {credentials.map((credential) => (
                <Text key={credential.provider} component="p">
                  {credential.provider} / {credential.auth_type} / {credential.api_key_mask ?? '未配置'} / {credential.enabled ? '启用' : '停用'}
                </Text>
              ))}
            </div>
          </Space>
        </Card>
      </div>
    </main>
  );
}
```

- [ ] **Step 4: Wire route and nav**

Modify `web/src/admin/routes.tsx`:

```tsx
import { MarketFeedConfigPage } from './actions/MarketFeedConfigPage';
```

Add route after market strategy actions:

```tsx
{ path: 'market/feed-config', element: <MarketFeedConfigPage /> },
```

Modify `web/src/layouts/AdminLayout.tsx` 行情市场 children:

```tsx
{ path: '/admin/market/feed-config', label: '行情订阅' }
```

- [ ] **Step 5: Run GREEN frontend tests**

Run:

```bash
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/admin/actions/MarketFeedConfigPage.test.tsx src/admin/routes.test.tsx src/layouts/AdminLayout.test.tsx
```

Expected: PASS.

---

## Task 7: Full verification and progress record

**Files:**
- Modify: `docs/superpowers/PROGRESS.md`

- [ ] **Step 1: Run backend formatting and checks**

Run:

```bash
cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check
cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets
cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings
```

Expected: all PASS.

- [ ] **Step 2: Run focused backend tests**

Run:

```bash
DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_market_feed -- --nocapture
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_feed_worker market_feed_supervisor -- --nocapture
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib admin::market_feed_config::tests config::tests::settings_from_env_parses_market_feed_lists -- --nocapture
```

Expected: all PASS.

- [ ] **Step 3: Run frontend verification**

Run:

```bash
npm run typecheck --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"
npm run lint --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"
npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/admin/actions/MarketFeedConfigPage.test.tsx src/admin/routes.test.tsx src/layouts/AdminLayout.test.tsx
```

Expected: all PASS.

- [ ] **Step 4: Append progress record**

Append to `docs/superpowers/PROGRESS.md`:

```markdown
## 2026-05-30 HH:mm - Admin 行情订阅后台配置与手动重载

- 完成内容：新增 MySQL `market_feed_configs` 和 `market_source_credentials`；新增 Admin 行情订阅配置、凭证掩码/加密保存、手动重载和运行状态 API；新增 market feed supervisor handle；接入 Admin 前端“行情订阅”页面，可保存订阅配置、保存行情源凭证并手动重载订阅。
- 修改文件：
  - `Cargo.toml`
  - `.env`
  - `migrations/0034_market_feed_admin_config.sql`
  - `src/config.rs`
  - `src/state.rs`
  - `src/main.rs`
  - `src/workers/market_feed.rs`
  - `src/modules/admin/mod.rs`
  - `src/modules/admin/market_feed_config.rs`
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `tests/market_feed_worker.rs`
  - `web/src/admin/actions/MarketFeedConfigPage.tsx`
  - `web/src/admin/actions/MarketFeedConfigPage.test.tsx`
  - `web/src/admin/routes.tsx`
  - `web/src/layouts/AdminLayout.tsx`
  - `web/src/layouts/AdminLayout.test.tsx`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：<填入实际执行命令和结果>
- 后续事项：如需验证真实第三方长连接，需在依赖服务启动后执行后端 smoke 并观察行情订阅日志。
```

- [ ] **Step 5: Final status**

Report:

- Implemented files.
- Verification commands and pass/fail status.
- Whether real WebSocket smoke was run.
- Any follow-up items.

---

## Self-Review

- Spec coverage: covered MySQL config table, encrypted credential table, Admin APIs, manual reload, runtime status, Admin frontend, audit, timestamp serialization, and verification.
- Placeholder scan: no `TBD`, `TODO`, `FIXME`, `待定`, or `占位` tokens are intentionally included.
- Type consistency: uses `MarketFeedRuntimeConfig`, `MarketFeedSupervisorHandle`, `MarketFeedRuntimeStatus`, `SaveMarketFeedConfigRequest`, `ReloadMarketFeedRequest`, `UpsertMarketSourceCredentialRequest`, and `MarketFeedConfigPage` consistently.
- Scope: implementation is broad but cohesive; all pieces are required for the user's explicit “并且接入管理员后台前端” request. The plan remains one feature slice with backend + frontend integration.
