# Manual New-Coin Subscription Distribution

## 1. Scope / Trigger

Applies to subscription creation, Admin order-linked distribution, subscription
reads, supply conservation and listing readiness. Post-listing direct purchases
retain immediate settlement. Project-center editing, actual listing gates and shared unlock-worker policy
are specified in [New-Coin Project Center](new-coin-project-center.md).

## 2. Signatures

- `POST /api/v1/new-coins/:symbol/subscriptions`: existing quantity, quote asset,
  quote amount and idempotency fields; success now returns `status=pending` and
  `lock_position_id=null` for a newly accepted subscription.
- `POST /admin/api/v1/new-coins/:id/distribute`:
  `{user_id, subscription_id?, quantity, idempotency_key, reason}`.
- Subscription read DTOs add `settlement_mode`, `frozen_quote_amount`,
  `settled_quote_amount`, and `refunded_quote_amount`. Admin also returns the
  original `issue_price` snapshot. Monetary fields are Decimal JSON text;
  historical settled/refunded amounts are null, not reconstructed guesses.
- Immutable migrations: `0121_new_coin_manual_distribution.sql` and
  `0122_new_coin_refund_receipt.sql`.

## 3. Contracts

- New subscriptions explicitly write `manual_distribution`. Existing rows
  retain `legacy_instant`; do not freeze, refund, or reissue historical holdings.
- Admission preserves the existing whole-request supply cap; it reserves quota
  but does not guarantee a final full allocation. There is no oversubscription
  pool, lottery or automatic distribution when subscription closes.
- A subscription transaction locks its project, reads a committed replay without
  a missing-key gap lock, validates authoritative price/asset/precision, reserves
  supply, inserts a pending order, and transfers quote available to frozen.
  It creates neither base holdings nor lock/unlock records. Cross-project key
  races resolve through the unique constraint and a post-rollback replay read.
- Admin confirms the **final** quantity once per manual order. Validate
  `0 <= quantity <= requested_quantity`. At zero, return all frozen funds and
  store a `refunded` receipt with no coin/lock entry. Unlinked grants still
  require positive quantity and do not settle a subscription.
- Actual payment uses the order's immutable issue price, not a changed project
  price. Preserve exact amount-precision validation; reject a partial quantity
  whose payment cannot be represented by the quote asset. Refund equals original
  frozen amount minus actual payment and returns to the same quote asset.
- Lock asset and wallet identities in stable order. Validate the order's frozen
  obligation as well as the aggregate wallet bucket; never spend another order's
  freeze. Payment, refund, base delivery/lock, supply release/finalization,
  terminal subscription state, receipt, lifecycle event and audit commit together.
- Conservation: `quote_amount = frozen_quote_amount + settled_quote_amount +
  refunded_quote_amount`; terminal manual orders have zero outstanding freeze.
  `total_supply = reserved_supply + allocated_supply + remaining_supply` remains
  unchanged. Undelivered quota stays reserved; unused quota returns to remaining;
  delivered quantity alone enters the existing allocated-supply counter.
- Full/partial/zero settlement uses `allocated` / `partial_allocated` / `refunded`.
  Partial is terminal for the manual order, not an invitation to send the rest.
- Same distribution key and same project/user/subscription/quantity returns the
  original receipt, including after listing. Changed inputs or another key for
  an already settled manual order conflict without another ledger or audit.
- New freeze writes paired `new_coin_subscription_freeze` ledger legs. Settlement
  payment debits frozen with `new_coin_subscription_payment`; refunds have paired
  `new_coin_subscription_refund` legs. Coin delivery uses existing distribution
  metadata. Preserve exact bucket snapshots and zero net movement on transfers.
- Listing is rejected while any manual subscription is pending or has an
  outstanding frozen amount. Stage changes never implicitly distribute orders.

## 4. Validation & Error Matrix

| Condition | Result |
| --- | --- |
| Wrong stage, quantity above request, negative quantity or unsupported precision | Reject without money/supply changes |
| Insufficient available balance at subscription | Roll back order, quota and freeze |
| Wrong subscription project/user | Not found; no settlement |
| Broken order freeze snapshot or insufficient aggregate frozen bucket | Conflict; no settlement |
| Zero quantity without a pending manual subscription | Reject; no zero-valued grant |
| Missing/blank audit reason | Validation error before database access |
| Same key / same business inputs | Return original distribution receipt |
| Same key / different business inputs or already-settled order / new key | Conflict |
| Pending manual order when advancing to listed | Conflict |
| Receipt/ledger/lock/audit write failure | Roll back all effects of the confirmation |

## 5. Good / Base / Bad Cases

- Base: request 10 at 2.5 freezes 25; final quantity 10 consumes 25 with no refund.
- Good: final quantity 4 consumes 10, returns 15, delivers/locks 4 and releases 6
  units of unused quota. Other business frozen balances remain untouched.
- Good: quantity 0 refunds 25, creates a refund receipt and no lock position.
- Bad: treating legacy fully paid orders as new frozen obligations or allowing
  another partial delivery after the difference has already been refunded.

## 6. Tests Required

- Pure amount tests: full, partial, zero, negative, excess, changed snapshot and
  nonrepresentable precision.
- Real MySQL route tests: subscription freeze/replay with no base allocation;
  supply competition; Admin full/partial/zero settlement, same-key concurrency,
  changed-key/input conflict, unrelated frozen-fund preservation, ledger/supply
  conservation, pending-listing guard, and same-key replay after listing.
- Inject a user-scoped failing distribution receipt trigger in a disposable
  database to prove rollback of money, supply, order and lock records. Use raw
  SQL for trigger DDL; remove the trigger before assertions/continuing the test.
- Existing legacy grants, rule changes, reads and direct purchases still pass.
- UI tests: authoritative order selection, precise preview and quantity payload,
  zero/excess/negative handling, legacy order exclusion and refresh after success.

## 7. Wrong vs Correct

Wrong: debit quote available and credit new coins at subscription, then call the
same entitlement a pending distribution; or debit the wallet's aggregate frozen
amount without checking which order owns it.

Correct: reserve the accepted order and quote funds first, then perform exactly
one explicit Admin confirmation which atomically delivers its final quantity and
returns its unused payment and quota.
