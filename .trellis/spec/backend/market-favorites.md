# Market Favorites and Asset Logo Contract

## Scope

This contract covers authenticated user market favorites, public market logo
metadata, and margin-wallet asset logo metadata. It does not change PC-local
favorites or introduce a third-party coin-image service.

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

Logo values are database-owned nullable strings. Backend handlers must not
derive an image path from a symbol, replace a missing value with a hard-coded
coin icon, or call an external image provider.

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
- Margin wallet tests for `assets.logo_url` propagation.
