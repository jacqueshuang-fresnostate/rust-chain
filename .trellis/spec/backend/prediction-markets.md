# Prediction Market Contracts

## Scenario: Polymarket-sourced prediction markets with local virtual-asset betting

### 1. Scope / Trigger

- Trigger: any change to the prediction market module, its database tables, Polymarket sync worker, PC prediction pages, admin prediction resources, wallet settlement code, or user-visible prediction order display.
- The external provider is a market and resolution source only. Local quotes, orders, wallet holds, fees, payouts, refunds, and audit fields are the platform source of truth.
- MVP supports Polymarket-synced markets only. Keep `source`, `external_event_id`, and `external_market_id` so future local/admin-created markets can reuse the same order and settlement model without changing PC/admin contracts.

### 2. Signatures

- Migration tables:
  - `prediction_settings` singleton row `id = 1`
  - `prediction_asset_configs`
  - `prediction_markets`
  - `prediction_sync_logs`
  - `prediction_quotes`
  - `prediction_orders`
- User API routes:
  - `GET /api/v1/prediction/config`
  - `GET /api/v1/prediction/markets`
  - `GET /api/v1/prediction/markets/:id`
  - `POST /api/v1/prediction/quotes`
  - `POST /api/v1/prediction/orders`
  - `GET /api/v1/prediction/orders`
- Admin API routes:
  - `GET /admin/api/v1/prediction/settings`
  - `PATCH /admin/api/v1/prediction/settings`
  - `GET /admin/api/v1/prediction/asset-configs`
  - `POST /admin/api/v1/prediction/asset-configs`
  - `PATCH /admin/api/v1/prediction/asset-configs/:asset_id`
  - `GET /admin/api/v1/prediction/markets`
  - `PATCH /admin/api/v1/prediction/markets/:id`
  - `POST /admin/api/v1/prediction/markets/:id/settle`
  - `GET /admin/api/v1/prediction/orders`
  - `GET /admin/api/v1/prediction/orders/:id`
  - `POST /admin/api/v1/prediction/sync`
  - `GET /admin/api/v1/prediction/sync/logs`
- Quote request:
  - `market_id: number`
  - `outcome: "yes" | "no"`
  - `asset_id: number`
  - `stake_amount: decimal string`
- Order request:
  - `quote_id: string`
  - `idempotency_key: string`
- Order display:
  - Backend field: `prediction_orders.order_no`
  - Business prefix: `PM`
  - PC adapter field: `orderNo`
- Wallet ledger:
  - `ref_type = "prediction_order"`
  - Change types: `prediction_stake_freeze`, `prediction_fee`, `prediction_settle_win`, `prediction_settle_loss`, `prediction_payout`, `prediction_stake_refund`, `prediction_fee_refund`.

### 3. Contracts

- Sync reads Polymarket Gamma `events` for both active discovery (`active=true`, `closed=false`) and closed-result reconciliation (`closed=true`), with `limit=100` and configured tags. Numeric tags use `tag_id`; non-numeric tags may use `tag_slug`.
- Events may embed `markets`. Flatten each event's `markets` array and merge event context such as event id, slug, category, image, and tags into each local `prediction_markets` row.
- Sync may update prices, status, external resolution, and payload snapshots. It must not directly move user funds except through the local settlement path.
- A closed upstream market without an unambiguous result must be hidden and moved to `pending_confirmation`; it must never remain externally open for new bets.
- User-visible markets only include active display status and open or pending-confirmation settlement status.
- User prediction routes protected by `UserAuth` must parse `Claims.sub` with the shared `user:<id>` subject shape used by sa-token sessions; raw numeric subjects should be rejected as unauthorized.
- A quote is backend-issued, bound to user, market, outcome, asset, stake amount, accepted price, fee, shares, payout cap, and expiry. Quotes are single-use and short-lived.
- An order must consume a valid quote and be idempotent by `(user_id, idempotency_key)`. It freezes stake amount and debits fee before returning a successful order.
- `stake_amount`, `fee_amount`, `shares`, payout, and refund amounts must obey the configured asset precision rules before touching wallet balances or ledger snapshots.
- Settlement results are `yes`, `no`, or `invalid`. Win/loss/invalid settlement must use one wallet path so manual settlement and auto settlement are idempotent and consistent.
- Settlement mode defaults to global `prediction_settings.default_settlement_mode` and may be overridden per market by `prediction_markets.settlement_mode_override`.
- Invalid market refund behavior defaults to `prediction_settings.default_invalid_refund_policy`; the concrete policy used must be copied to order/market fields such as `invalid_refund_policy_used`.
- Effective allowed assets are global `prediction_settings.allowed_asset_ids_json` unless `prediction_markets.allowed_asset_ids_override_json` is present.
- Effective payout cap is per-asset `prediction_asset_configs.max_payout_amount` unless overridden by `prediction_markets.payout_cap_overrides_json`.
- Effective fee rate is global `prediction_settings.default_fee_rate` unless `prediction_markets.fee_rate_override` is present.
- Admin prediction asset config lists may left-join all active `assets` before an asset has a prediction config row. The base `assets` table only guarantees `created_at`; do not select `assets.updated_at` as a fallback timestamp.
- Admin tables should show user email, market title, asset symbol, order number, and Chinese labels. They should not expose raw quote id, user id, market id, or asset id as primary display columns unless needed for a technical detail view.
- PC pages must display `orderNo` for prediction orders, not raw `id`.
- PC user prediction order lists must keep the market title in the expanded detail row instead of showing it as a primary table column.
- PC market lists must localize dynamic market text. If the API includes locale documents such as `title_i18n_json`, `description_i18n_json`, `category_i18n_json`, or outcome label i18n fields, the current locale wins. If Polymarket only supplies English text, the PC page must apply a local fallback for common prediction-market phrases so Chinese users do not see an all-English market list.
- PC prediction discovery must expose a Polymarket-style list/detail contract: `/prediction` is the category/topic discovery page and `/prediction/:id` is the market detail page. The detail page must load `GET /api/v1/prediction/markets/:id` and still route all staking through backend-issued quotes.
- PC prediction order tickets must calculate the max stake from the selected asset balance and the effective payout cap times the selected outcome price. Market `payout_cap_overrides_json[asset_id]` wins over the asset default; an override value of `0` is a valid unlimited cap and must not fall back to the asset default. Any PC display of an effective payout cap less than or equal to zero must show the localized unlimited label instead of `0`.

### 4. Validation & Error Matrix

- Missing MySQL pool -> return the standard API database availability error.
- Disabled sync -> scheduled worker exits without calling Polymarket; manual admin sync may still run only if the route explicitly allows it.
- Polymarket non-2xx or malformed response -> admin sync returns/logs `POLYMARKET_SYNC_FAILED`; do not panic the server.
- Market inactive, closed, already settled, or refunded -> quote creation fails before any wallet operation.
- `outcome` not `yes`/`no` -> validation error.
- `stake_amount <= 0` or amount precision exceeds asset precision -> validation error.
- Asset not globally allowed or not allowed by market override -> validation error.
- Asset config disabled or missing -> validation error.
- Theoretical payout exceeds effective cap where cap is greater than zero -> validation error.
- Admin prediction asset config list references `assets.updated_at` -> MySQL 1054 unknown-column error on the current schema; use `configs.updated_at` or fall back to `assets.created_at`.
- Quote missing, expired, consumed, or owned by another user -> validation error before wallet mutation.
- Insufficient wallet available balance for stake or fee -> wallet error and no order insert should be committed.
- Manual invalid refund policy selected but no concrete policy supplied at settlement time -> validation error.
- External invalid result plus manual invalid refund policy -> set market to pending confirmation instead of auto-refunding.
- Replayed idempotency key -> return the existing order with `changed = false`.

### 5. Good/Base/Bad Cases

- Good: sync imports an active Polymarket event with two markets, PC requests a quote for `yes`, backend signs the price and cap, the order consumes the quote once, freezes stake, debits fee, and later settlement pays or refunds through wallet ledger entries.
- Base: external resolution arrives while global settlement mode is `manual_confirm`; market enters `pending_confirmation`, admin reviews it in SideSheet, then calls the settle endpoint.
- Bad: PC calculates odds and submits a raw order without a backend quote, or admin auto-refunds an invalid market without recording which refund policy was used.

### 6. Tests Required

- Unit test the Polymarket event extraction path with event-level context and embedded markets.
- Route-prefix test must include user and admin prediction route groups.
- Backend check must include `cargo fmt`, `cargo check --all-targets`, and focused prediction tests for sync extraction and route registration.
- Wallet/settlement tests should assert stake freeze, fee debit, payout/refund ledger types, idempotency replay, expired quote rejection, and payout cap rejection when those paths are changed.
- Web admin typecheck and resource config tests must cover prediction resources and `PM` order-number display.
- PC typecheck and static tests must cover header/sidebar entries, route registration, adapter `orderNo` mapping, and user order tab rendering.
- PC localization tests must cover configured locale documents and common Polymarket fallback phrases such as Fed rate cuts, FDV after token launch, categories, and YES/NO outcomes.
- PC prediction page tests must cover `/prediction/:id` route registration, detail API usage, list-to-detail navigation, and the i18n keys needed by the category/detail experience.
- Migration validation should be run in an environment without prior checksum conflicts. If historical migration checksums are already dirty, document why `sqlx migrate run` was skipped and never edit old applied migrations.

### 7. Wrong vs Correct

#### Wrong

```rust
// A PC order payload chooses price directly and bypasses the backend quote table.
create_prediction_order(user_id, market_id, outcome, asset_id, stake_amount, frontend_price).await?;
```

```tsx
{ key: 'id', title: '订单ID' }
```

#### Correct

```rust
// The order consumes a backend-owned quote and all wallet changes happen in one transaction.
let quote = lock_quote(&mut tx, &request.quote_id).await?;
create_order_from_quote(&mut tx, user.id, quote, &request.idempotency_key).await?;
```

```tsx
orderNoColumn('PM')
```
