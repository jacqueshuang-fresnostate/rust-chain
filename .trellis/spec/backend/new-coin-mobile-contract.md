# New Coin Mobile Contract

## Scenario: User New-Coin Lifecycle on Mobile

### 1. Scope / Trigger
- Trigger: the mobile client needs to render a project detail page and call the post-listing purchase endpoint without guessing the admin-configured trading pair.
- Scope: public project reads, authenticated subscription and purchase writes, and authenticated lifecycle record and unlock actions.
- The backend keeps the route layer thin. `presentation` exposes the fields, `repository` declares the read shape, and `infrastructure` reads the existing columns from `new_coin_projects`.

### 2. Signatures
- `GET /api/v1/new-coins`
- `GET /api/v1/new-coins/:symbol`
- `POST /api/v1/new-coins/:symbol/subscriptions`
- `POST /api/v1/new-coins/:symbol/purchase`
- `GET /api/v1/new-coins/{subscriptions,distributions,purchases,unlocks}`
- `POST /api/v1/new-coins/unlocks/:id/pay-fee`
- `POST /api/v1/new-coins/unlocks/:id/release`

### 3. Contracts
- Project responses include `post_listing_purchase_enabled: boolean` and `post_listing_pair_id: number | null` in addition to lifecycle, issuance, unlock, and fee fields.
- The pair id is authoritative. A client may call the purchase endpoint only when `post_listing_purchase_enabled` is true and `post_listing_pair_id` is present.
- Subscription request fields: `quote_asset_id`, positive `quote_amount`, positive `quantity`, and a unique `idempotency_key`.
- Purchase request fields: the configured `pair_id`, positive `price`, positive `quantity`, and a unique `idempotency_key`.
- Unlock-fee request fields: `payment_asset_id` and the exact configured `amount`; release uses the unlock record's `idempotency_key` in the route path.

### 4. Validation & Error Matrix
| Condition | Result |
| --- | --- |
| Project is not in `subscription` lifecycle | Subscription request is rejected. |
| Project is not in `listed` lifecycle | Post-listing purchase is rejected. |
| Purchase pair differs from `post_listing_pair_id` | Purchase is rejected. |
| Price or quantity is zero/negative | Write request is rejected. |
| Unlock fee asset/amount differs from the configured rule | Fee payment is rejected. |
| Fee has not been paid or unlock time has not arrived | Release is rejected. |

### 5. Good / Base / Bad Cases
- Good: a listed project returns `post_listing_purchase_enabled: true` and an id; the mobile client submits that exact id and displays the returned purchase in its lifecycle records.
- Base: a subscription project returns `false` and `null`; the mobile client presents subscription only and never tries to infer a pair from the symbol.
- Bad: a client derives a pair id from a symbol or sends a stale pair id. This bypass attempt must be rejected by `ensure_post_listing_purchase_enabled`.

### 6. Tests Required
- `tests/new_coin_routes.rs` must assert that public project responses serialize `post_listing_purchase_enabled` and `post_listing_pair_id`.
- The route test suite must cover enabled/disabled post-listing pair validation, wallet debit, lock creation, fee payment, and due release.
- Mobile type checks must cover the response mapping; production build validates route-level lazy imports.

### 7. Wrong vs Correct
#### Wrong
```typescript
const pairId = markets.find((market) => market.base === project.symbol)?.id
```

This is ambiguous when a project has multiple markets and can disagree with the admin-approved pair.

#### Correct
```typescript
if (project.postListingPurchaseEnabled && project.postListingPairId) {
  await createNewCoinPurchase({ pairId: project.postListingPairId, ...input })
}
```

The UI uses the pair chosen by the backend while the application layer validates it again before wallet mutation.
