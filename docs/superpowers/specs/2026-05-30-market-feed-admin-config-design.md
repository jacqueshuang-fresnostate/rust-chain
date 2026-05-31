# Market Feed Admin Configuration Design

**Goal:** Move third-party market feed subscription settings from fixed `.env` lists into Admin-managed MySQL configuration with manual reload and encrypted provider credentials.

**Architecture:** Store subscription intent in MySQL, keep `.env` as bootstrap fallback, and separate saving configuration from applying it to the running worker. Admin reload explicitly validates the latest saved config, loads provider credentials, restarts market feed subscriptions, and records audit/status without exposing secrets.

**Tech Stack:** Rust + Axum, SQLx + MySQL, React + Semi Design, existing market feed worker, existing Admin auth/audit patterns.

---

## Decisions

- Use a new MySQL `market_feed_configs` table for subscription configuration.
- Use manual reload: saving config does not immediately affect the running worker.
- Add encrypted provider credential storage managed from Admin UI.
- Use `CREDENTIAL_ENCRYPTION_KEY` from environment as the encryption root key.
- Keep existing `.env` values as fallback only when no active DB config exists.
- Do not return plaintext API secrets from any API.

## Data Model

### `market_feed_configs`

One active config row is enough for the current platform-wide market feed.

| Column | Type | Purpose |
|---|---|---|
| `id` | `BIGINT UNSIGNED` | Primary key |
| `name` | `VARCHAR(64)` | Config name, e.g. `default` |
| `symbols_json` | `JSON` | Subscribed symbols, e.g. `["BTC-USDT","ETH-USDT"]` |
| `intervals_json` | `JSON` | K-line intervals, e.g. `["1m","5m","15m","1h","1d"]` |
| `providers_json` | `JSON` | Providers, e.g. `["bitget","htx"]` |
| `enabled` | `BOOLEAN` | Whether DB market feed config is enabled |
| `version` | `BIGINT UNSIGNED` | Incremented on every config save |
| `applied_version` | `BIGINT UNSIGNED NULL` | Version currently applied by worker |
| `last_reload_status` | `VARCHAR(32) NULL` | `success` / `failed` / `skipped` |
| `last_reload_error` | `TEXT NULL` | Last reload error without secrets |
| `last_reloaded_at` | `TIMESTAMP(6) NULL` | Last reload time; API returns Unix milliseconds |
| `updated_by` | `BIGINT UNSIGNED NULL` | Admin user id |
| `created_at` | `TIMESTAMP(6)` | Creation time |
| `updated_at` | `TIMESTAMP(6)` | Update time |

Rules:

- `symbols_json` must contain valid market symbols accepted by the existing market symbol validator.
- `intervals_json` must only contain supported intervals: `1m`, `5m`, `15m`, `1h`, `1d`.
- `providers_json` must only contain supported providers from the existing provider registry.
- Empty symbols are allowed only when `enabled = false`.
- Updating config increments `version` and leaves `applied_version` unchanged until reload succeeds.

### `market_source_credentials`

Stores optional provider credentials.

| Column | Type | Purpose |
|---|---|---|
| `id` | `BIGINT UNSIGNED` | Primary key |
| `provider` | `VARCHAR(32)` | `bitget`, `htx`, future provider code |
| `auth_type` | `VARCHAR(32)` | `none` / `api_key` |
| `api_key_ciphertext` | `TEXT NULL` | Encrypted API key |
| `api_secret_ciphertext` | `TEXT NULL` | Encrypted secret |
| `passphrase_ciphertext` | `TEXT NULL` | Encrypted passphrase when provider needs it |
| `api_key_mask` | `VARCHAR(64) NULL` | Safe display mask, e.g. `abcd****wxyz` |
| `enabled` | `BOOLEAN` | Whether this credential can be used |
| `updated_by` | `BIGINT UNSIGNED NULL` | Admin user id |
| `created_at` | `TIMESTAMP(6)` | Creation time |
| `updated_at` | `TIMESTAMP(6)` | Update time |

Rules:

- API responses return `api_key_mask`, never ciphertext or plaintext.
- Updating a credential requires sending the full new secret values; omitted secret fields keep existing encrypted values.
- Audit logs must not contain plaintext API key, secret, passphrase, or ciphertext.
- If `auth_type = none`, encrypted secret fields must be ignored.
- If `auth_type = api_key`, required credential parts are provider-specific.

## Admin API

Add routes under `/admin/api/v1`:

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/market-feed/config` | Read saved subscription config and reload status |
| `PATCH` | `/market-feed/config` | Save symbols, intervals, providers, enabled flag, reason |
| `POST` | `/market-feed/reload` | Manually reload running subscriptions from DB config |
| `GET` | `/market-feed/status` | Read active runtime status, applied version, last result |
| `GET` | `/market-feed/credentials` | List provider credential masks and enabled states |
| `PATCH` | `/market-feed/credentials/:provider` | Upsert encrypted credential for one provider |

All routes require `AdminAuth`.

### Save config flow

1. Validate request body.
2. Normalize symbols using existing market symbol normalization.
3. Validate intervals and providers.
4. Write `market_feed_configs` and increment `version`.
5. Write admin audit log with before/after config values, excluding secrets.
6. Return saved config and `needs_reload = version != applied_version`.

### Reload flow

1. Require a reason string.
2. Load active `market_feed_configs` row.
3. If disabled, stop running market feed and mark reload as `skipped` or `success` with no active subscriptions.
4. Validate symbols, intervals, providers again.
5. Load enabled credentials for selected providers and decrypt in memory only.
6. Build a new runtime config.
7. Start the new market feed loop only after config construction succeeds.
8. Stop old loop after the new config is accepted for startup.
9. Update `applied_version`, `last_reload_status`, `last_reload_error`, `last_reloaded_at`.
10. Write admin audit log with reload reason and sanitized result.

If reload fails before the new loop is accepted, keep the old loop running and record `failed`.

## Runtime Worker Changes

Add a small market feed supervisor handle to `AppState` or worker startup:

- Tracks the currently applied config version.
- Owns cancellation for the active market feed task.
- Exposes `reload(config)` for Admin API.
- Exposes `status()` for Admin API.

The existing env-driven startup becomes bootstrap logic:

1. Try to load enabled DB config.
2. If no enabled DB config exists, use `Settings.market_feed_symbols`, `Settings.market_feed_intervals`, and `Settings.market_feed_providers`.
3. If both are empty, keep existing disabled behavior and log that no symbols are configured.

## Admin UI

Add an Admin page under 行情市场:

- Title: `行情订阅配置`
- Fields:
  - `启用第三方行情订阅`
  - `订阅交易对`
  - `K 线周期`
  - `行情源`
  - `操作原因`
- Provider credential section:
  - Provider code
  - Auth type
  - API key mask
  - Enabled state
  - Edit credential drawer/modal
- Status section:
  - Saved version
  - Applied version
  - Whether reload is needed
  - Last reload status
  - Last reload time
  - Last reload error
- Actions:
  - `保存配置`
  - `重载行情订阅`

Secret handling in UI:

- Show only mask after save.
- Do not prefill secret/password fields.
- To rotate a secret, admin must input a new value.

## Security

- `CREDENTIAL_ENCRYPTION_KEY` must be required before saving `auth_type = api_key` credentials.
- Encryption and decryption errors return sanitized API errors.
- Logs and audit records must never include plaintext or ciphertext credentials.
- Admin reload and credential changes must include an operation reason.
- API key fields must be redacted in request/response debug logs if request logging is added later.

## Error Handling

- Invalid symbol, interval, or provider: reject save and reload with `400`.
- Missing encryption key while saving credentials: reject with server configuration error.
- Missing required provider credential part: reject reload for that provider.
- Third-party connection failure during reload: keep old worker active, mark reload failed, show sanitized reason.
- Disabled config: stop worker and record clean disabled status.

## Testing

Backend tests:

- Config save rejects invalid symbols, intervals, providers.
- Config save increments version and sets `needs_reload`.
- Reload applies valid DB config and updates `applied_version`.
- Reload failure keeps old runtime config.
- Credential API returns masks only.
- Credential audit excludes plaintext and ciphertext.
- Missing `CREDENTIAL_ENCRYPTION_KEY` rejects credential save.
- Env fallback still works when DB config is absent.

Frontend tests:

- Page renders saved config and status.
- Editing config calls `PATCH /admin/api/v1/market-feed/config`.
- Reload button calls `POST /admin/api/v1/market-feed/reload` with reason.
- Credential form never pre-fills secret values.
- Credential list shows API key mask only.

Verification commands:

```bash
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
npm run typecheck --prefix web
npm run lint --prefix web
npm run test --prefix web
```

## Progress Recording

After implementation, append a `docs/superpowers/PROGRESS.md` entry listing migrations, backend API, frontend page, tests, and any unverified runtime smoke limitations.
