# Market Favorites and Asset Logo Contract

## Scope

This contract covers authenticated user market favorites, public market and
convert-pair logo metadata, and margin-wallet asset logo metadata. It does not
change PC-local favorites or introduce a third-party coin-image service.

## HTTP Contract

```text
GET    /api/v1/user/market-favorites
PUT    /api/v1/user/market-favorites/:symbol
DELETE /api/v1/user/market-favorites/:symbol
```

- Every endpoint requires `UserAuth` and operates only on the authenticated
  user ID.
- `:symbol` is parsed with the existing market-symbol validator and resolved
  only against an active trading pair.
- PUT is idempotent: adding an existing `(user_id, trading_pair_id)` succeeds
  without creating a duplicate.
- DELETE is idempotent: deleting an absent favorite succeeds.
- GET returns only favorites whose trading pair is still active.

## Persistence Contract

`user_market_favorites` owns the relationship between users and trading pairs.

- `(user_id, trading_pair_id)` is unique.
- Both foreign keys use `ON DELETE CASCADE` so user or trading-pair removal
  cannot leave orphaned favorites.
- Queries must bind the authenticated user ID; a symbol or favorite ID alone
  is never sufficient authorization.

## Response Contract

Favorite records expose:

```text
market_id
symbol
logo_url
base_logo_url
quote_logo_url
base_asset
quote_asset
```

The public market response preserves `trading_pairs.logo_url` and also exposes
`base_logo_url` and `quote_logo_url` from the joined asset rows. Margin wallet
rows expose `logo_url` from `assets.logo_url`.

The public convert-pairs response exposes `from_asset_logo_url` and
`to_asset_logo_url` from the corresponding joined `assets.logo_url` rows. Both
fields remain present as JSON `null` when the database value is absent.

Logo values are database-owned nullable strings. Backend handlers must not
derive an image path from a symbol, replace a missing value with a hard-coded
coin icon, or call an external image provider.

## Scenario: Public Convert-Pair Asset Logos

### 1. Scope / Trigger

- Trigger: adding or changing asset-image metadata on `GET /api/v1/convert/pairs`.
- Scope: the public pair DTO and the existing `convert_pairs -> assets` joins;
  quote, confirmation, order, and admin contracts are unchanged.

### 2. Signatures

```text
GET /api/v1/convert/pairs
from_asset_logo_url: string | null
to_asset_logo_url: string | null
```

### 3. Contracts

- `from_asset_logo_url` is the exact nullable `logo_url` of the joined
  `from_assets` row.
- `to_asset_logo_url` is the exact nullable `logo_url` of the joined
  `to_assets` row.
- Both JSON keys remain present when their values are `null`.
- The endpoint remains public and preserves its existing ordering, enabled
  filter, limit, and all pre-existing response fields.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| Both assets have Logo values | Return each value under its direction-specific key |
| One or both assets have no Logo | Keep the affected key present with JSON `null` |
| Asset row is absent | Existing inner-join behavior; no partial pair row |
| MySQL is unavailable | Existing convert-route internal error |

### 5. Good / Base / Bad Cases

- Good: two different database Logo values survive SQLx and JSON serialization
  unchanged.
- Base: an absent Logo is represented by `Option<String>::None` and JSON
  `null`.
- Bad: deriving `/coins/{symbol}.png`, substituting a bundled image, or using
  one side's Logo for both directions.

### 6. Tests Required

- A no-database serialization test must assert both configured values and both
  keys as JSON `null`.
- A database-backed route test must set distinct from/to Logo values, assert
  the complete HTTP response, and include a pair whose two Logo values are
  absent.
- Existing symbol and pair fields must remain asserted to detect accidental
  contract replacement.

### 7. Wrong vs Correct

Wrong:

```rust
let logo = format!("/assets/{}.png", asset_symbol);
```

Correct:

```sql
SELECT from_assets.logo_url AS from_asset_logo_url,
       to_assets.logo_url AS to_asset_logo_url
```

## Error Matrix

| Condition | Required result |
| --- | --- |
| Missing or invalid user session | Existing authentication error |
| Invalid market symbol syntax | Validation error |
| Unknown or inactive trading pair | Not-found error |
| Duplicate PUT | Success with one persisted row |
| Repeated DELETE | Success with no row |
| Another user's favorite exists | No visibility and no mutation of that row |

## Required Verification

- Migration source test for the unique key and both cascading foreign keys.
- Route authentication and invalid-symbol tests.
- Database-backed CRUD, duplicate PUT, repeated DELETE, user isolation, active
  filtering, and user/trading-pair cascade tests when a test database is
  available.
- Public market tests for pair/base/quote logo fields.
- Public convert-pair serialization and database route tests for configured and
  null from/to asset logo fields.
- Margin wallet tests for `assets.logo_url` propagation.
