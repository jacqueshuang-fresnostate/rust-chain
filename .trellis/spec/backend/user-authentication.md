# User Authentication Contracts

## Scenario: User username configuration and username login policy

### 1. Scope / Trigger

- Trigger: any change to user login identifiers, user profile identity fields, security policy login toggles, or PC login payload construction.
- Scope: `migrations/`, `src/modules/auth`, `src/modules/security`, `src/modules/user`, admin security-policy APIs, OpenAPI docs, and PC login/security-center clients.

### 2. Signatures

- DB:
  - `users.username VARCHAR(64) NULL UNIQUE`
  - `security_policy_configs.policy_value.username_login_enabled: boolean`
  - `security_policy_configs.policy_value` may contain legacy boolean representations from old migrations or MySQL JSON functions: `true/false`, `1/0`, or `"1"/"0"`.
- Public auth API:
  - `GET /api/v1/auth/login/config -> { "username_login_enabled": boolean }`
  - `POST /api/v1/auth/login` accepts `email`, `phone`, or `username` plus `password`.
- User API:
  - `GET /api/v1/user/profile` returns `username: string | null`.
  - `PATCH /api/v1/user/username` request `{ "username": string }`, response `{ "username": string }`.
- Admin API:
  - `GET/PATCH /admin/api/v1/security-policy` includes `username_login_enabled`.

### 3. Contracts

- Usernames are login identifiers, not display names. Normalize them to lowercase ASCII before storing or lookup.
- Valid username format: 3-32 characters, ASCII letters, ASCII digits, or underscore.
- `users.username` stays nullable for existing accounts, and the unique index allows multiple NULL values.
- Username login is disabled by default. Old security-policy JSON must deserialize with `username_login_enabled=false`.
- `load_security_policy` must read `policy_value` as `serde_json::Value`, normalize security-policy boolean fields, then deserialize `UserSecurityPolicy`. Do not query the JSON column directly as `SqlxJson<UserSecurityPolicy>`, because legacy `"0"` / `"1"` values break endpoints that read the policy.
- Security-policy boolean normalization applies to `enabled`, keys ending in `_enabled`, and `registration_invite_required`. Saving the policy still writes canonical JSON booleans.
- Email and phone login remain available regardless of the username-login policy.
- PC login should read `/auth/login/config`; when the switch is enabled, an input without `@` may be submitted as `username`.
- PC profile/header display should prefer `profile.username`, then email, then phone, then user id.

### 4. Validation & Error Matrix

- Missing email/phone/username during login -> `400 validation error`.
- Username login request while `username_login_enabled=false` -> `400 validation error`.
- Invalid username format on update/login lookup -> `400 validation error`.
- Duplicate username update -> `409 conflict`.
- Wrong password, inactive user, or unknown username -> `401 unauthorized`.
- Wrong auth scope for profile/username update -> `403 forbidden`.
- Legacy security policy JSON containing `"0"`, `"1"`, `0`, or `1` for boolean fields -> normalize to boolean before response; do not return `DATABASE_ERROR` from `/api/v1/user/2fa`, `/api/v1/user/third-party-bindings`, `/api/v1/auth/login`, or admin security-policy routes.

### 5. Good/Base/Bad Cases

- Good: admin enables username login, user sets `Moon_1024`, backend stores `moon_1024`, and PC can log in with `Moon_1024` because the service normalizes lookup.
- Base: existing user has `username=NULL`; profile returns null and PC displays email.
- Bad: PC always sends non-email input as username without checking `/auth/login/config`; this causes confusing validation failures when username login is disabled.
- Bad: using username as a mutable display nickname; future display-name needs a separate field.
- Bad: decoding `security_policy_configs.policy_value` directly into `SqlxJson<UserSecurityPolicy>`; historical JSON values like `"0"` cause `expected a boolean` decode failures.

### 6. Tests Required

- Auth unit tests for username normalization and policy-gated username credential verification.
- User route tests for profile `username` and `PATCH /user/username` round-trip.
- Auth route tests for `/auth/login/config`, username-login disabled, and username-login enabled.
- Admin security-policy tests for `username_login_enabled` persistence and audit JSON.
- PC adapter/static tests for profile username priority, login config wiring, and security-center username update wiring.
- OpenAPI tests for new fields and routes.
- Security module unit tests must cover legacy string/numeric boolean policy JSON, including third-party binding switches and payment-policy `enabled` fields.

### 7. Wrong vs Correct

#### Wrong

```rust
// Accepts arbitrary Unicode and lets a disabled policy still query by username.
let stored = repository.find_user_by_username(input_username).await?;
```

#### Correct

```rust
let identifier = user_login_identifier(email, phone, username, policy.username_login_enabled)?;
let stored = match identifier {
    UserLoginIdentifier::Username(username) => repository.find_user_by_username(&username).await?,
    // email and phone paths unchanged
};
```

#### Wrong

```rust
let policy = sqlx::query_scalar::<_, SqlxJson<UserSecurityPolicy>>(
    "SELECT policy_value FROM security_policy_configs WHERE policy_key = ?",
)
.fetch_optional(pool)
.await?;
```

#### Correct

```rust
let policy_json = sqlx::query_scalar::<_, SqlxJson<serde_json::Value>>(
    "SELECT policy_value FROM security_policy_configs WHERE policy_key = ?",
)
.fetch_optional(pool)
.await?;
let policy = policy_json
    .map(|value| decode_security_policy_value(value.0))
    .transpose()?
    .unwrap_or_default();
```
