# Aggregate Exposure Admission

## Scope

This contract covers Loan principal, Earn product principal/gross liability,
Convert explicit output inventory budgets, and seconds gross payout capacity.
It does not define custody balances, external funding, FX conversion, a shared
capital pool, interest/fee changes, approval policy, or automatic liquidation.

Applied migrations are immutable:

- `0133_loan_exposure_limits.sql`
- `0136_earn_convert_exposure.sql`
- `0138_seconds_payout_capacity.sql`

## Configuration And Defaults

| Policy | Unit and scope | Inactive default |
| --- | --- | --- |
| Loan `user_principal_limit` | User principal across all Loan products in the candidate asset | null |
| Loan `product_principal_capacity` | Candidate product principal in its asset | null |
| Loan `deny_borrowing_while_overdue` | Existing Loan overdue debt blocks new borrowing | false |
| Earn `principal_capacity` | Product subscribed principal in its asset | null |
| Earn `liability_capacity` | Product subscribed principal plus full-term gross yield | null |
| Convert inventory | Pair's configured forward output asset; explicit funded budget minus consumed budget | missing or disabled |
| Seconds `open_payout_capacity` | Product gross payout obligation in its stake asset | null |

Non-null monetary limits are nonnegative DECIMAL(38,18) values that must fit the
configured asset precision. Zero forbids additional positive exposure. Never
sum unlike currencies, insert invented opening funds, or silently truncate
submitted caps. Admin amounts remain exact decimal strings. Blank nullable
limits become JSON null; zero must survive form roundtrip.

## Transaction And Lifecycle Rules

### Loan

- Pending orders reserve principal. Disbursed and overdue orders consume
  outstanding principal. Cancelled, rejected, repaid and liquidated terminal
  statuses release it.
- Creation and approval use dedicated user serialization rows without user
  foreign keys, then the product, candidate order and existing financial locks.
- Approval reads current policy and rechecks tightened limits, excluding the
  candidate from aggregate sums before adding it back once.
- Do not lock all aggregate orders or establish a consistent snapshot before
  the admission serialization locks. Repay/reject/health workflows must not
  acquire the admission locks in reverse order.
- Existing idempotent replay precedes new admission policy.

### Earn

- Lock the product first; only then establish the ordinary read snapshot and
  repeat the same-key lookup. Product edits use the same row lock.
- Aggregate only `subscribed` rows for that product and asset.
- Each liability reservation equals principal plus full-term gross yield at
  the subscription's captured APR and duration. Round the gross-yield reserve
  upward to 18 decimals; do not deduct possible future fees.
- Redemption releases capacity through the final status. Aggregation does not
  lock subscriptions, so concurrent redemption can temporarily overcount but
  cannot admit more than the cap.
- Changing a product asset with incompatible outstanding subscriptions fails.
- This liability budget does not prove liquid funds exist.

### Convert

- `convert_inventory_accounts` holds only an operator-configured budget and an
  atomic consumption counter. Neither journal net nor external holdings
  provide an inferred initial or replenishment balance.
- The main confirmation transaction locks quote, pair/configuration and assets
  before calling `consume_output_in_tx`, and calls it before order/wallet
  settlement. Consumption plus quote-unique allocation commits with wallet and
  platform journal effects, or rolls back with them.
- Quotes do not reserve inventory. A quote is not a guarantee that the output
  budget will still be available at confirmation.
- Configure funding with expected revision, explicit total, funding reference
  and authenticated reason. A stale revision or total below consumed funds is
  rejected. Disabling preserves funds, consumption and immutable history.
- Public duplicate confirmation retains its established conflict response and
  cannot consume budget again. Helper replay checks allocation parameters.
- The current account is pair-unique and binds the forward output asset.
  Protection explicitly rejects reverse-direction confirmation; it must never
  use the forward currency's budget to pay another currency. The Admin form
  discloses both this restriction and that configured budget is not verified
  custody. Missing/disabled policy preserves legacy bidirectional operation.
- A future pair-and-asset account migration is needed for independently funded
  reverse output. Do not edit applied migration 0136 to retrofit it.
- Inventory history prevents changing its output currency or deleting the pair.

### Seconds

- Lock product before the first ordinary snapshot, then check exact replay
  before capacity, history capability, ticker lookup and insertion.
- Aggregate `opened` and `manual_review` orders for the same product and stake
  asset. Manual review is still an unpaid obligation, not released capital.
- Each order reserves its maximum actual gross payout:
  `stake * (1 + captured net payout rate)`, rounded upward to 18 decimals.
  The candidate uses its selected cycle's rate. Later rate edits never change
  old reservations. Do not guess that rates above one are gross factors.
- Product edits and admissions serialize on the same product row. Aggregation
  does not lock other orders or alter settlement/refund lock order.
- Settled orders release capacity. A successful principal refund releases it
  only through the atomic `refunded` terminal transition; its receipt, original
  debit evidence, wallet, platform journal and audit are owned by the refund
  workflow. Do not create a speculative release for a waiting/refund request.
- Product stake-asset changes fail while incompatible unpaid obligations exist.
- Refund eligibility is prospective: while still holding the product lock,
  call `infrastructure::refund::snapshot_refund_policy_in_tx` only after a new
  order insert succeeds, in the same opening transaction before wallet debit.
  Missing/disabled policy makes no snapshot. Replays never create or refresh
  eligibility; enabling later cannot qualify legacy orders.

## Required Evidence

- Real MySQL concurrent small admissions must accept exactly the configured
  boundary, not merely avoid obvious single-order overflow.
- Cover null/zero, changed limits, immutable rate/term snapshots, exact replay
  at full capacity, released terminal exposure and asset isolation.
- Loan approval must recheck a tightened cap without double counting.
- Convert tests must prove explicit budget/audit revision, no invented funds,
  concurrent confirmations, wallet-failure rollback and no duplicate
  consumption; test forward-only refusal while enabled.
  Its optional real-DB lib test uses only `CONVERT_INVENTORY_DATABASE_URL`,
  guarded to loopback TCP and a dedicated `_test` database. Do not read the
  generic `DATABASE_URL` concurrently mutated by configuration unit tests.
- Seconds tests must retain manual-review exposure and prove original
  snapshots survive a product-rate change, settled/refunded capacity releases,
  and prospective refund snapshots do not retrofit old orders.
- Admin tests cover exact decimal roundtrip, null clearing, zero retention,
  malformed/overflow/asset-precision input, reason and revision where applicable.
- Run scoped route/unit tests, Web typecheck and focused form tests, backend
  architecture guards, all-target compile, formatting and diff checks.
  Record unavailable services or concurrent-worktree blockers explicitly.

## Operational Boundaries

All four policies are product/admission controls, not a consolidated solvency
or custody system. Tightening a cap never rewrites historical obligations,
creates cash, initiates sweeping, retroactively changes fees, or liquidates
positions. Global asset capital pools and verified external funding require a
separately specified model and operational evidence.
