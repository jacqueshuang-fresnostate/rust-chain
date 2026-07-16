# 用户 2FA 与后台安全策略 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build TOTP 2FA binding/reset, Admin-configured login/payment security policy, login 2FA challenge, and withdrawal security verification across Rust backend, Admin React frontend, and PC Vue frontend.

**Architecture:** Add a focused Rust `security` module for policy parsing, TOTP generation/verification, encrypted TOTP secret access, login challenge persistence, and reusable action verification. Keep user-facing 2FA endpoints in user/auth routes, Admin policy/reset endpoints in Admin routes, and integrate the first payment-action enforcement through a minimal wallet withdrawal request endpoint. Frontends consume normalized adapters so UI pages remain thin.

**Tech Stack:** Rust + Axum + sqlx + MySQL, existing `hmac`/`sha1`/`base64`/`ring` dependencies for TOTP and randomness, React + Semi Design + Vitest for Admin, Vue 3 + Pinia + node:test for PC.

**Constraints:** Do not commit unless the user explicitly asks. Do not store TOTP secrets in plaintext. Keep changes limited to the approved 2FA/security-policy scope. Use TDD: write/extend tests first, verify RED, implement, verify GREEN.

---

## File Structure

### Backend

- Create: `migrations/0043_user_2fa_security_policy.sql`
  - `user_two_factor_settings`
  - `security_policy_configs`
  - `login_two_factor_challenges`
  - `wallet_withdrawal_requests`
- Create: `src/modules/security.rs`
  - Security policy enums/defaults/validation.
  - TOTP secret generation, base32 encode/decode, otpauth URI creation, TOTP verification.
  - SQL helpers for reading/saving policy, two-factor settings, login challenges, and verifying payment actions.
- Modify: `src/modules/mod.rs`
  - Export `security`.
- Modify: `src/error.rs`
  - Add API error variant that can return exact error codes required by the design.
- Modify: `src/modules/auth/mod.rs`
  - Add a credential verification path that returns the authenticated user actor without issuing tokens immediately.
- Modify: `src/modules/auth/routes.rs`
  - Return either token, `requires_2fa`, or `requires_2fa_setup` from `/auth/login`.
  - Add `/auth/login/2fa`, `/auth/login/2fa/reset-code`, `/auth/login/2fa/reset`.
- Modify: `src/modules/user/routes.rs`
  - Add `/user/2fa`, `/user/2fa/setup`, `/user/2fa/confirm`, `/user/2fa/login`, `/user/2fa/reset-code`, `/user/2fa/reset`.
- Modify: `src/modules/admin/mod.rs`
  - Export Admin security policy module if split from routes.
- Modify: `src/modules/admin/routes.rs`
  - Add `/security-policy` get/patch and `/users/:id/2fa/reset`.
- Modify: `src/modules/wallet/routes.rs`
  - Add minimal `/wallet/withdrawals` POST using security verification helper before recording request.
- Modify: `src/openapi.rs`
  - Add user/auth/Admin/wallet 2FA and security policy OpenAPI paths/schemas.
- Test: `tests/user_routes.rs`, `tests/auth_routes.rs` or existing auth test module, `tests/admin_routes.rs`, `tests/wallet_routes.rs`, `tests/openapi_routes.rs`.

### Admin frontend

- Create: `web/src/admin/actions/SecurityPolicyPage.tsx`
  - Login 2FA policy dropdown and payment policy table.
- Create: `web/src/admin/actions/SecurityPolicyPage.test.tsx`
  - Page/API payload tests.
- Modify: `web/src/api/client.ts` only if shared helper types are needed.
- Modify: `web/src/admin/routes.tsx`
  - Add `/admin/system/security-policy`.
- Modify: `web/src/layouts/AdminLayout.tsx`
  - Add “安全策略” entry under system configuration.
- Modify: `web/src/admin/resources/resourceConfigs.tsx` or user resource actions if needed.
  - Add user row action “重置 2FA”.
- Test: existing route/layout/resource config tests.

### PC frontend

- Modify: `pc/src/api/backendAdapters.ts`
  - Add auth challenge, 2FA status, security policy, and withdrawal security adapter types.
- Modify: `pc/tests/backendAdapters.test.ts`
  - Add adapter and static wiring tests.
- Modify: `pc/src/api/auth.ts`
  - Add login 2FA challenge API calls.
- Modify: `pc/src/api/user.ts`
  - Add user 2FA API calls.
- Modify: `pc/src/api/wallet.ts`
  - Replace unavailable withdraw stub with Rust backend call and security fields.
- Modify: `pc/src/views/auth/Login.vue`
  - Add 2FA challenge step and mandatory setup step.
- Modify: `pc/src/views/User/Security.vue`
  - Add 2FA bind/reset/login-switch block.
- Modify: `pc/src/views/User/Withdraw.vue`
  - Show security verification fields based on policy summary and submit them with withdrawal.

---

### Task 1: Backend schema and exact API error codes

**Files:**
- Create: `migrations/0043_user_2fa_security_policy.sql`
- Modify: `src/error.rs`
- Test: `tests/openapi_routes.rs` for route-path placeholders in later task; this task uses compile checks after code is wired.

- [ ] **Step 1: Write migration**

Create `migrations/0043_user_2fa_security_policy.sql` with:

```sql
CREATE TABLE IF NOT EXISTS user_two_factor_settings (
    user_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    totp_secret_encrypted TEXT NULL,
    totp_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    login_2fa_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    confirmed_at TIMESTAMP NULL,
    last_verified_at TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT fk_user_two_factor_settings_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS security_policy_configs (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    policy_key VARCHAR(64) NOT NULL UNIQUE,
    policy_value JSON NOT NULL,
    updated_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_security_policy_configs_updated_by (updated_by)
);

INSERT INTO security_policy_configs (policy_key, policy_value)
VALUES (
    'user_security_policy',
    JSON_OBJECT(
        'login_2fa_mode', 'user_enabled',
        'payment_policies', JSON_OBJECT(
            'withdraw', JSON_OBJECT('enabled', TRUE, 'method', 'fund_password'),
            'spot_order', JSON_OBJECT('enabled', FALSE, 'method', 'fund_password'),
            'convert', JSON_OBJECT('enabled', FALSE, 'method', 'fund_password'),
            'earn_subscribe', JSON_OBJECT('enabled', FALSE, 'method', 'fund_password')
        )
    )
)
ON DUPLICATE KEY UPDATE policy_key = policy_key;

CREATE TABLE IF NOT EXISTS login_two_factor_challenges (
    challenge_id CHAR(36) NOT NULL PRIMARY KEY,
    user_id BIGINT UNSIGNED NOT NULL,
    challenge_type VARCHAR(32) NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    consumed_at TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_login_two_factor_challenges_user (user_id),
    INDEX idx_login_two_factor_challenges_expires_at (expires_at),
    CONSTRAINT fk_login_two_factor_challenges_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS wallet_withdrawal_requests (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_symbol VARCHAR(32) NOT NULL,
    network VARCHAR(64) NULL,
    address VARCHAR(255) NOT NULL,
    amount DECIMAL(36, 18) NOT NULL,
    fee DECIMAL(36, 18) NOT NULL DEFAULT 0,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    security_method VARCHAR(64) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_wallet_withdrawal_requests_user (user_id),
    INDEX idx_wallet_withdrawal_requests_status (status),
    CONSTRAINT fk_wallet_withdrawal_requests_user FOREIGN KEY (user_id) REFERENCES users(id)
);
```

- [ ] **Step 2: Add coded API error variant**

Modify `src/error.rs` to add a variant like:

```rust
Api {
    status: StatusCode,
    code: &'static str,
    message: String,
},
```

Update `IntoResponse` so this variant emits:

```json
{ "code": "invalid_2fa_code", "message": "2FA 验证码错误" }
```

Add helper constructors on `AppError`:

```rust
pub fn security_validation(code: &'static str, message: impl Into<String>) -> Self
pub fn security_forbidden(code: &'static str, message: impl Into<String>) -> Self
```

- [ ] **Step 3: Verify formatting after the first compile-capable wiring**

Run after Task 2 wires the module:

```bash
cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" -- --check
```

Expected: PASS.

---

### Task 2: Security policy and TOTP core module

**Files:**
- Create: `src/modules/security.rs`
- Modify: `src/modules/mod.rs`
- Test: add unit tests inside `src/modules/security.rs` under `#[cfg(test)]`.

- [ ] **Step 1: Write RED tests for policy defaults and TOTP**

Add tests to `src/modules/security.rs` before implementing bodies:

```rust
#[test]
fn default_security_policy_requires_fund_password_for_withdraw_only() {
    let policy = UserSecurityPolicy::default();
    assert_eq!(policy.login_2fa_mode, LoginTwoFactorMode::UserEnabled);
    assert_eq!(policy.payment_policies.withdraw.enabled, true);
    assert_eq!(policy.payment_policies.withdraw.method, SecurityVerificationMethod::FundPassword);
    assert_eq!(policy.payment_policies.convert.enabled, false);
}

#[test]
fn totp_matches_rfc_6238_sha1_vector() {
    let secret = b"12345678901234567890";
    assert_eq!(totp_code_for_time(secret, 59, 30, 6), "287082");
    assert_eq!(totp_code_for_time(secret, 1111111109, 30, 6), "081804");
}

#[test]
fn base32_roundtrip_preserves_random_secret_bytes() {
    let bytes = b"exchange-2fa-secret";
    let encoded = base32_encode_no_padding(bytes);
    assert_eq!(base32_decode_no_padding(&encoded).unwrap(), bytes);
}
```

- [ ] **Step 2: Run RED test**

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" default_security_policy_requires_fund_password_for_withdraw_only -- --nocapture
```

Expected: FAIL because `src/modules/security.rs` and the types do not exist yet.

- [ ] **Step 3: Implement minimal core types and TOTP helpers**

Implement in `src/modules/security.rs`:

```rust
pub const USER_SECURITY_POLICY_KEY: &str = "user_security_policy";
pub const LOGIN_CHALLENGE_TTL_SECONDS: i64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginTwoFactorMode {
    None,
    UserEnabled,
    Mandatory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityVerificationMethod {
    FundPassword,
    TwoFactor,
    FundPasswordAndTwoFactor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityAction {
    Withdraw,
    SpotOrder,
    Convert,
    EarnSubscribe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentPolicy {
    pub enabled: bool,
    pub method: SecurityVerificationMethod,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentPolicies {
    pub withdraw: PaymentPolicy,
    pub spot_order: PaymentPolicy,
    pub convert: PaymentPolicy,
    pub earn_subscribe: PaymentPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSecurityPolicy {
    pub login_2fa_mode: LoginTwoFactorMode,
    pub payment_policies: PaymentPolicies,
}
```

Use `Hmac<Sha1>` to implement HOTP/TOTP and a local RFC4648 base32 encoder/decoder. Generate new user secrets as 20 random bytes with `ring::rand::SystemRandom` and return an uppercase base32 string without padding.

- [ ] **Step 4: Implement SQL helpers**

Add functions:

```rust
pub async fn load_security_policy(pool: &Pool<MySql>) -> AppResult<UserSecurityPolicy>;
pub async fn save_security_policy(pool: &Pool<MySql>, policy: &UserSecurityPolicy, admin_id: u64) -> AppResult<()>;
pub async fn load_user_two_factor(pool: &Pool<MySql>, user_id: u64) -> AppResult<UserTwoFactorSettings>;
pub async fn save_pending_totp_secret(pool: &Pool<MySql>, user_id: u64, encrypted_secret: &str) -> AppResult<()>;
pub async fn confirm_user_totp(pool: &Pool<MySql>, user_id: u64, encrypted_secret: &str) -> AppResult<()>;
pub async fn reset_user_two_factor(pool: &Pool<MySql>, user_id: u64) -> AppResult<()>;
```

All TOTP secret encrypt/decrypt call sites must use `encrypt_secret` / `decrypt_secret` with `Settings::exposed_credential_encryption_key()`.

- [ ] **Step 5: Run GREEN tests**

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib security -- --nocapture
```

Expected: PASS.

---

### Task 3: User 2FA API

**Files:**
- Modify: `src/modules/user/routes.rs`
- Modify: `src/modules/security.rs`
- Test: `tests/user_routes.rs`

- [ ] **Step 1: Write RED tests for user 2FA endpoints**

Add tests to `tests/user_routes.rs` using existing route test helpers:

```rust
#[tokio::test]
async fn user_two_factor_status_route_is_registered() {
    let app = build_test_app_without_mysql();
    let response = request_json(&app, Method::GET, "/api/v1/user/2fa", None).await;
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn user_two_factor_setup_requires_encryption_key_when_mysql_is_available() {
    let Some(database_url) = database_url() else { return; };
    let app = build_mysql_test_app_without_credential_key(&database_url).await;
    let token = create_test_user_token(&app).await;
    let response = authed_json(&app, Method::POST, "/api/v1/user/2fa/setup", &token, json!({})).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
```

If helper names differ, adapt to existing `tests/user_routes.rs` helpers but preserve the assertions and paths.

- [ ] **Step 2: Run RED route test**

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test user_routes user_two_factor_status_route_is_registered -- --nocapture
```

Expected: FAIL with 404 before routes are added.

- [ ] **Step 3: Add user 2FA route handlers**

Add to `routes()`:

```rust
.route("/user/2fa", get(get_two_factor_status))
.route("/user/2fa/setup", post(setup_two_factor))
.route("/user/2fa/confirm", post(confirm_two_factor))
.route("/user/2fa/login", patch(update_login_two_factor))
.route("/user/2fa/reset-code", post(send_two_factor_reset_code))
.route("/user/2fa/reset", post(reset_two_factor))
```

Request/response shapes:

```rust
#[derive(Debug, Serialize)]
struct UserTwoFactorStatusResponse {
    totp_enabled: bool,
    login_2fa_enabled: bool,
    login_2fa_mode: LoginTwoFactorMode,
    can_toggle_login_2fa: bool,
    payment_policies: PaymentPolicies,
}

#[derive(Debug, Serialize)]
struct SetupTwoFactorResponse {
    secret: String,
    otpauth_uri: String,
}

#[derive(Debug, Deserialize)]
struct ConfirmTwoFactorRequest {
    totp_code: String,
}

#[derive(Debug, Deserialize)]
struct UpdateLoginTwoFactorRequest {
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct ResetTwoFactorRequest {
    code: String,
}
```

Behavior:
- `/user/2fa` returns settings and current policy summary.
- `/user/2fa/setup` rejects already enabled with `2fa_already_enabled`; otherwise generates a secret, encrypts it, saves pending secret, and returns secret + otpauth URI.
- `/user/2fa/confirm` decrypts pending secret, verifies TOTP, then sets `totp_enabled = true` and timestamps.
- `/user/2fa/login` only works when policy is `user_enabled`; enabling requires bound TOTP; otherwise return `login_2fa_policy_locked` or `2fa_not_enabled`.
- reset code endpoints reuse existing email verification table/pattern with a purpose distinct from fund password, for example `two_factor_reset`.

- [ ] **Step 4: Run GREEN user-route tests**

```bash
env -u DATABASE_URL cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test user_routes -- --nocapture
```

Expected: PASS; MySQL-specific tests skip when `DATABASE_URL` is unset.

---

### Task 4: Login 2FA challenge flow

**Files:**
- Modify: `src/modules/auth/mod.rs`
- Modify: `src/modules/auth/routes.rs`
- Modify: `src/modules/security.rs`
- Test: auth route tests or `tests/user_routes.rs` if auth tests are consolidated there.

- [ ] **Step 1: Write RED tests for login challenge route registration and response shape**

Add tests:

```rust
#[tokio::test]
async fn login_two_factor_route_is_registered() {
    let app = build_test_app_without_mysql();
    let response = request_json(&app, Method::POST, "/api/v1/auth/login/2fa", Some(json!({
        "challenge_id": "00000000-0000-0000-0000-000000000000",
        "totp_code": "123456"
    }))).await;
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn login_two_factor_reset_routes_are_registered() {
    let app = build_test_app_without_mysql();
    let send = request_json(&app, Method::POST, "/api/v1/auth/login/2fa/reset-code", Some(json!({
        "challenge_id": "00000000-0000-0000-0000-000000000000"
    }))).await;
    assert_ne!(send.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run RED tests**

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" login_two_factor_route_is_registered -- --nocapture
```

Expected: FAIL with 404 before route is added.

- [ ] **Step 3: Add credential verification without immediate token issuance**

In `src/modules/auth/mod.rs`, add a method that verifies password and returns the user actor:

```rust
pub async fn verify_user_credentials(&self, credentials: UserCredentials) -> AppResult<AuthActor>;
```

Keep existing `login_user` behavior by calling this method then issuing tokens. Do not weaken status checks.

- [ ] **Step 4: Add login challenge responses**

In `src/modules/auth/routes.rs`, replace `Json<TokenResponse>` login response with an untagged enum:

```rust
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum UserLoginResponse {
    Token(TokenResponse),
    TwoFactorChallenge(LoginTwoFactorChallengeResponse),
    TwoFactorSetupChallenge(LoginTwoFactorSetupChallengeResponse),
}
```

Implement policy behavior:
- `none`: issue token.
- `user_enabled`: if user has `totp_enabled && login_2fa_enabled`, create `login_2fa` challenge and return `requires_2fa`.
- `mandatory`: if bound, create `login_2fa`; if not bound, create `setup_2fa` and return `requires_2fa_setup`.

- [ ] **Step 5: Add challenge verification/reset handlers**

Add routes:

```rust
.route("/auth/login/2fa", post(user_login_two_factor))
.route("/auth/login/2fa/reset-code", post(send_login_two_factor_reset_code))
.route("/auth/login/2fa/reset", post(reset_login_two_factor))
```

Handlers:
- Verify unexpired, unconsumed challenge.
- Verify TOTP against encrypted user secret.
- Mark challenge consumed before issuing tokens.
- For reset-code/reset, send/check email code scoped to the challenge user; reset 2FA and require re-login, not token issuance.

- [ ] **Step 6: Run GREEN auth tests**

```bash
env -u DATABASE_URL cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test user_routes -- --nocapture
```

Expected: PASS; MySQL-specific tests skip without `DATABASE_URL`.

---

### Task 5: Payment security verification helper and withdrawal endpoint

**Files:**
- Modify: `src/modules/security.rs`
- Modify: `src/modules/wallet/routes.rs`
- Test: `tests/wallet_routes.rs` or create if absent.

- [ ] **Step 1: Write RED tests for security verification route behavior**

Add tests:

```rust
#[tokio::test]
async fn wallet_withdrawals_route_is_registered() {
    let app = build_test_app_without_mysql();
    let response = request_json(&app, Method::POST, "/api/v1/wallet/withdrawals", Some(json!({
        "asset_symbol": "USDT",
        "network": "TRC20",
        "address": "TTestAddress",
        "amount": "10",
        "fee": "1",
        "fund_password": "123456"
    }))).await;
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}
```

For MySQL-enabled tests, add cases:
- default policy requires fund password for withdraw.
- two_factor policy rejects unbound user with code `2fa_required_not_bound`.
- dual policy rejects missing field with code `security_verification_required`.

- [ ] **Step 2: Run RED route test**

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" wallet_withdrawals_route_is_registered -- --nocapture
```

Expected: FAIL with 404 before route is added.

- [ ] **Step 3: Implement reusable action verification**

Add to `src/modules/security.rs`:

```rust
pub struct SecurityVerificationInput<'a> {
    pub fund_password: Option<&'a str>,
    pub totp_code: Option<&'a str>,
}

pub async fn verify_user_security_action(
    pool: &Pool<MySql>,
    settings: &Settings,
    user_id: u64,
    action: SecurityAction,
    input: SecurityVerificationInput<'_>,
) -> AppResult<SecurityVerificationMethod>;
```

Behavior:
- Disabled policy passes.
- Fund password checks `user_security.fund_password_hash` with existing `verify_password`.
- TOTP checks bound secret and returns `2fa_required_not_bound` if missing.
- Missing fields return `security_verification_required`.
- Wrong TOTP returns `invalid_2fa_code`.

- [ ] **Step 4: Implement minimal withdrawal route**

In `src/modules/wallet/routes.rs`, add:

```rust
.route("/wallet/withdrawals", post(create_withdrawal_request))
```

Request:

```rust
struct CreateWithdrawalRequest {
    asset_symbol: String,
    network: Option<String>,
    address: String,
    amount: Decimal,
    fee: Decimal,
    fund_password: Option<String>,
    totp_code: Option<String>,
}
```

Response:

```rust
struct WithdrawalRequestResponse {
    id: u64,
    status: String,
    security_method: SecurityVerificationMethod,
}
```

This first phase records a pending withdrawal request after successful security verification. It must not fake success without backend persistence.

- [ ] **Step 5: Run GREEN wallet tests**

```bash
env -u DATABASE_URL cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test wallet_routes -- --nocapture
```

Expected: PASS or MySQL tests skip when `DATABASE_URL` is unset.

---

### Task 6: Admin security policy and Admin reset 2FA APIs

**Files:**
- Modify: `src/modules/admin/routes.rs`
- Optional create: `src/modules/admin/security_policy.rs`
- Modify: `src/modules/admin/mod.rs`
- Test: `tests/admin_routes.rs`

- [ ] **Step 1: Write RED Admin route tests**

Add tests:

```rust
#[tokio::test]
async fn admin_security_policy_route_is_registered() {
    let app = build_test_app_without_mysql();
    let response = request_json(&app, Method::GET, "/admin/api/v1/security-policy", None).await;
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_user_two_factor_reset_route_is_registered() {
    let app = build_test_app_without_mysql();
    let response = request_json(&app, Method::POST, "/admin/api/v1/users/1/2fa/reset", Some(json!({
        "reason": "用户申请重置 2FA"
    }))).await;
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run RED tests**

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_security_policy_route_is_registered -- --nocapture
```

Expected: FAIL with 404 before route is added.

- [ ] **Step 3: Add Admin routes**

Add:

```rust
.route("/security-policy", get(get_security_policy).patch(update_security_policy))
.route("/users/:id/2fa/reset", post(reset_admin_user_two_factor))
```

Policy PATCH validates:
- `login_2fa_mode` is one of `none`, `user_enabled`, `mandatory`.
- Every action has `enabled` boolean and valid method.
- Payment action keys are exactly `withdraw`, `spot_order`, `convert`, `earn_subscribe`.

Both PATCH policy and reset 2FA must insert `admin_audit_logs` rows.

- [ ] **Step 4: Run GREEN Admin tests**

```bash
env -u DATABASE_URL cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture
```

Expected: PASS; MySQL-specific tests skip when `DATABASE_URL` is unset.

---

### Task 7: OpenAPI contract

**Files:**
- Modify: `src/openapi.rs`
- Test: `tests/openapi_routes.rs`

- [ ] **Step 1: Write RED OpenAPI assertions**

Extend `openapi_json_exposes_first_batch_contract` with path assertions:

```rust
assert!(paths.contains_key("/api/v1/user/2fa"));
assert!(paths.contains_key("/api/v1/user/2fa/setup"));
assert!(paths.contains_key("/api/v1/user/2fa/confirm"));
assert!(paths.contains_key("/api/v1/user/2fa/login"));
assert!(paths.contains_key("/api/v1/user/2fa/reset-code"));
assert!(paths.contains_key("/api/v1/user/2fa/reset"));
assert!(paths.contains_key("/api/v1/auth/login/2fa"));
assert!(paths.contains_key("/api/v1/auth/login/2fa/reset-code"));
assert!(paths.contains_key("/api/v1/auth/login/2fa/reset"));
assert!(paths.contains_key("/api/v1/wallet/withdrawals"));
assert!(paths.contains_key("/admin/api/v1/security-policy"));
assert!(paths.contains_key("/admin/api/v1/users/{id}/2fa/reset"));
```

- [ ] **Step 2: Run RED OpenAPI test**

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test openapi_routes openapi_json_exposes_first_batch_contract -- --nocapture
```

Expected: FAIL until OpenAPI is updated.

- [ ] **Step 3: Add OpenAPI paths and schemas**

Add schemas for:
- `UserTwoFactorStatusResponse`
- `SetupTwoFactorResponse`
- `ConfirmTwoFactorRequest`
- `UpdateLoginTwoFactorRequest`
- `LoginTwoFactorChallengeResponse`
- `LoginTwoFactorVerifyRequest`
- `UserSecurityPolicy`
- `CreateWithdrawalRequest`
- `WithdrawalRequestResponse`
- `AdminResetTwoFactorRequest`

- [ ] **Step 4: Run GREEN OpenAPI test**

```bash
cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test openapi_routes openapi_json_exposes_first_batch_contract -- --nocapture
```

Expected: PASS.

---

### Task 8: PC adapters and API clients

**Files:**
- Modify: `pc/tests/backendAdapters.test.ts`
- Modify: `pc/src/api/backendAdapters.ts`
- Modify: `pc/src/api/auth.ts`
- Modify: `pc/src/api/user.ts`
- Modify: `pc/src/api/wallet.ts`

- [ ] **Step 1: Write RED adapter tests**

Add tests:

```ts
test('normalizes login 2FA challenge response', () => {
  const response = normalizeAuthResponse({
    requires_2fa: true,
    challenge_id: 'challenge-1',
    expires_in_seconds: 300,
  })

  assert.equal(response.code, 0)
  assert.equal(response.data.requiresTwoFactor, true)
  assert.equal(response.data.challengeId, 'challenge-1')
})

test('normalizes mandatory 2FA setup challenge response', () => {
  const response = normalizeAuthResponse({
    requires_2fa_setup: true,
    setup_challenge_id: 'setup-1',
    expires_in_seconds: 300,
  })

  assert.equal(response.data.requiresTwoFactorSetup, true)
  assert.equal(response.data.setupChallengeId, 'setup-1')
})

test('normalizes user two factor status', () => {
  const status = normalizeTwoFactorStatus({
    totp_enabled: true,
    login_2fa_enabled: false,
    login_2fa_mode: 'user_enabled',
    can_toggle_login_2fa: true,
    payment_policies: {
      withdraw: { enabled: true, method: 'fund_password_and_two_factor' },
      spot_order: { enabled: false, method: 'fund_password' },
      convert: { enabled: false, method: 'fund_password' },
      earn_subscribe: { enabled: false, method: 'fund_password' },
    },
  })

  assert.equal(status.totpEnabled, true)
  assert.equal(status.paymentPolicies.withdraw.method, 'fund_password_and_two_factor')
})
```

- [ ] **Step 2: Run RED PC adapter tests**

```bash
node --experimental-strip-types --test "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/pc/tests/backendAdapters.test.ts"
```

Expected: FAIL because new adapter exports do not exist.

- [ ] **Step 3: Implement adapters**

Add types and functions:

```ts
export function normalizeAuthResponse(payload: BackendAuthTokenResponse | BackendLoginChallengeResponse): PcAuthResponse
export function normalizeTwoFactorStatus(payload: BackendTwoFactorStatusResponse): PcTwoFactorStatus
export function mapWithdrawRequest(params: WithdrawParams): BackendWithdrawRequest
```

Keep existing token-login response shape compatible.

- [ ] **Step 4: Implement PC API calls**

`pc/src/api/auth.ts`:

```ts
export async function loginTwoFactor(challengeId: string, totpCode: string)
export async function sendLoginTwoFactorResetCode(challengeId: string)
export async function resetLoginTwoFactor(challengeId: string, code: string)
```

`pc/src/api/user.ts`:

```ts
export async function getTwoFactorStatus()
export async function setupTwoFactor()
export async function confirmTwoFactor(totpCode: string)
export async function updateLoginTwoFactor(enabled: boolean)
export async function sendTwoFactorResetCode()
export async function resetTwoFactor(code: string)
```

`pc/src/api/wallet.ts`:

```ts
export async function submitWithdraw(params: WithdrawParams)
```

Call `backendApiUrl('/wallet/withdrawals')` and include `fund_password` / `totp_code` when present.

- [ ] **Step 5: Run GREEN PC adapter tests**

```bash
node --experimental-strip-types --test "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/pc/tests/backendAdapters.test.ts"
```

Expected: PASS.

---

### Task 9: PC Login, Security, and Withdraw UI

**Files:**
- Modify: `pc/src/views/auth/Login.vue`
- Modify: `pc/src/views/User/Security.vue`
- Modify: `pc/src/views/User/Withdraw.vue`
- Test: `pc/tests/backendAdapters.test.ts` static wiring checks plus type-check/build.

- [ ] **Step 1: Add static wiring RED checks**

In `pc/tests/backendAdapters.test.ts`, add file-content checks matching existing style:

```ts
test('login page handles backend 2FA challenge states', () => {
  const source = readFileSync(new URL('../src/views/auth/Login.vue', import.meta.url), 'utf8')
  assert.match(source, /requiresTwoFactor/)
  assert.match(source, /loginTwoFactor/)
  assert.match(source, /requiresTwoFactorSetup/)
})

test('security page exposes user 2FA actions', () => {
  const source = readFileSync(new URL('../src/views/User/Security.vue', import.meta.url), 'utf8')
  assert.match(source, /setupTwoFactor/)
  assert.match(source, /confirmTwoFactor/)
  assert.match(source, /resetTwoFactor/)
  assert.match(source, /updateLoginTwoFactor/)
})

test('withdraw page submits security verification fields', () => {
  const source = readFileSync(new URL('../src/views/User/Withdraw.vue', import.meta.url), 'utf8')
  assert.match(source, /fundPassword/)
  assert.match(source, /totpCode/)
})
```

- [ ] **Step 2: Run RED static tests**

```bash
node --experimental-strip-types --test "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/pc/tests/backendAdapters.test.ts"
```

Expected: FAIL until UI wiring exists.

- [ ] **Step 3: Implement Login 2FA step**

In `Login.vue`:
- Keep existing password-login form.
- If `normalizeAuthResponse` returns token, call existing session/profile/wallet success path.
- If `requiresTwoFactor`, switch to TOTP input step and call `loginTwoFactor`.
- If `requiresTwoFactorSetup`, show forced setup message and setup/confirm controls using the setup challenge path or tell the user to bind and re-login according to backend response.
- Add reset link for challenge reset-code/reset flow; after reset, clear challenge and require password login again.

- [ ] **Step 4: Implement Security 2FA block**

In `Security.vue`:
- Load `getTwoFactorStatus()` alongside existing security profile.
- Unbound: show “未绑定 2FA” and bind button.
- Bound: show “已绑定 2FA” and reset button.
- `user_enabled`: show login 2FA switch.
- `none`: show current policy does not require login 2FA.
- `mandatory`: show platform requires login 2FA; no close entry.
- Bind modal displays `otpauth_uri`, manual `secret`, and TOTP code field.
- Reset modal sends email code and calls reset.

- [ ] **Step 5: Implement Withdraw security fields**

In `Withdraw.vue`:
- Load user 2FA status/policy or use policy summary from status.
- For withdraw policy:
  - `fund_password`: show fund password input.
  - `two_factor`: show TOTP input.
  - `fund_password_and_two_factor`: show both.
- Submit fields through `submitWithdraw`.
- If backend returns `2fa_required_not_bound`, show message and route/link user to Security page.

- [ ] **Step 6: Run PC type-check and build**

```bash
npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/pc" run type-check
npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/pc" run build
```

Expected: PASS.

---

### Task 10: Admin frontend security policy page and reset action

**Files:**
- Create: `web/src/admin/actions/SecurityPolicyPage.tsx`
- Create: `web/src/admin/actions/SecurityPolicyPage.test.tsx`
- Modify: `web/src/admin/routes.tsx`
- Modify: `web/src/layouts/AdminLayout.tsx`
- Modify: `web/src/admin/resources/resourceConfigs.tsx` or user action config file.
- Test: `web/src/admin/routes.test.tsx`, `web/src/layouts/AdminLayout.test.tsx`, resource config tests.

- [ ] **Step 1: Write RED Admin frontend tests**

Add tests:

```tsx
it('submits security policy payload with login mode and payment policies', async () => {
  const requests: Array<{ path: string; init?: RequestInit }> = [];
  render(<SecurityPolicyPage request={captureRequest(requests)} />);
  await screen.findByText('安全策略');
  await userEvent.click(screen.getByRole('button', { name: '保存策略' }));
  expect(requests.some(request => request.path === '/admin/api/v1/security-policy')).toBe(true);
});
```

Add static route/layout assertions:

```ts
expect(routePaths).toContain('/admin/system/security-policy')
expect(layoutText).toContain('安全策略')
```

- [ ] **Step 2: Run RED Admin frontend tests**

```bash
npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- SecurityPolicyPage
```

Expected: FAIL because page does not exist.

- [ ] **Step 3: Implement SecurityPolicyPage**

Page contents:
- `PageHeader` title: `安全策略`.
- Login 2FA mode `AdminSelect`:
  - `none` = `不要求`
  - `user_enabled` = `用户开启时要求`
  - `mandatory` = `全站强制要求`
- Payment policy table with rows:
  - `withdraw` = `提现`
  - `convert` = `闪兑`
  - `spot_order` = `现货下单`
  - `earn_subscribe` = `理财申购`
- Each row has enabled switch/checkbox and method select:
  - `fund_password` = `资金密码`
  - `two_factor` = `2FA`
  - `fund_password_and_two_factor` = `资金密码 + 2FA`
- Save via PATCH `/admin/api/v1/security-policy`.

- [ ] **Step 4: Wire route and layout**

Add route:

```tsx
{ path: 'system/security-policy', element: <SecurityPolicyPage /> }
```

Add sidebar item under system configuration:

```ts
{ label: '安全策略', path: '/admin/system/security-policy' }
```

- [ ] **Step 5: Add Admin user row reset action**

Add row action `重置 2FA` on user management rows/details. It calls:

```ts
apiRequest(`/admin/api/v1/users/${user.id}/2fa/reset`, {
  method: 'POST',
  body: JSON.stringify({ reason }),
})
```

Require a confirmation reason using existing `ConfirmAction` pattern.

- [ ] **Step 6: Run GREEN Admin frontend tests**

```bash
npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test
```

Expected: PASS.

---

### Task 11: Full verification and progress record

**Files:**
- Modify: `docs/superpowers/PROGRESS.md`

- [ ] **Step 1: Run backend verification**

```bash
cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" -- --check
env -u DATABASE_URL cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test user_routes --test admin_routes --test openapi_routes --test wallet_routes -- --nocapture
cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings
```

Expected: PASS. If no `DATABASE_URL`, MySQL integration branches should skip by test logic; record this explicitly.

- [ ] **Step 2: Run Admin frontend verification**

```bash
npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test
npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck
npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run build
```

Expected: PASS; record any existing build warnings exactly.

- [ ] **Step 3: Run PC frontend verification**

```bash
node --experimental-strip-types --test "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/pc/tests/backendAdapters.test.ts"
npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/pc" run type-check
npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/pc" run build
```

Expected: PASS.

- [ ] **Step 4: Run diff whitespace check**

```bash
git diff --check -- "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain"
```

Expected: PASS.

- [ ] **Step 5: Update progress log**

Append to `docs/superpowers/PROGRESS.md`:

```markdown
## 2026-06-12 HH:mm - 用户 2FA 与后台安全策略实现

- 完成内容：完成用户 TOTP 2FA 绑定/确认/重置，登录 2FA challenge，Admin 安全策略配置与用户 2FA 重置，提现安全校验闭环，PC 登录/安全设置/提现接入，Admin 安全策略页面和用户重置动作。
- 修改文件：<列出实际修改文件>
- 验证结果：<列出实际执行命令和结果>
- 后续事项：<没有则写“无”；如 MySQL 未连通则写明真实集成验证条件>
```

- [ ] **Step 6: Cancel progress reminder**

After the whole task is verified and recorded, cancel the session-only progress reminder job:

```text
CronDelete id d2123b13
```

Expected: progress reminder stops after task completion.
