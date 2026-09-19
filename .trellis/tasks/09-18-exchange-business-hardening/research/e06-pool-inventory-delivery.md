# E06 Earn, Convert And Seconds Exposure

## Scope And Policy

- Migration `0136_earn_convert_exposure.sql` adds nullable Earn product principal
  and gross liability capacities, plus explicit Convert funding accounts,
  immutable funding audits, and quote-unique consumption allocations.
- Earn limits default to null (inactive); zero closes new subscriptions.
  Product principal includes all `subscribed` amounts in that product/asset.
  Gross liability is each subscription's principal plus full-term gross yield,
  using its captured APR and term, rounded upward to 18 decimal places.
  Future fees do not reduce this conservative budget. This is not a cash pool.
- Earn holds the product row before the first ordinary snapshot read, including
  a same-key replay check. Admission and product edits serialize on that row.
  Aggregation does not lock other subscriptions. Concurrent redemptions can
  temporarily overcount capacity, but cannot cause over-admission.
  Asset changes are rejected while incompatible active subscriptions exist.
- Convert has no inferred opening balance: only explicit administrator-funded
  totals count. Missing/disabled policy does not constrain existing behavior.
  Configuration requires an expected revision, funding reference, and reason.
  Reducing total funding below consumed funds is rejected. Disabling does not
  erase funding, consumption, or audit history.
- Main integrated `inventory::consume_output_in_tx` after authoritative quote
  validation and before order/wallet writes. The existing quote/pair lock order
  precedes the inventory lock. A committed confirmation consumes the exact net
  output amount; later wallet/journal/transaction failure rolls back consumption
  and the allocation. Quotes themselves do not reserve funds.
- Inventory helper replay is idempotent. The public confirmation API preserves
  its existing 409 response for an already consumed quote, without a second
  inventory or wallet effect.
- The current account is per pair and binds the configured forward output
  asset. With protection enabled, reverse-direction output in another asset is
  rejected, not funded from the forward asset balance. A future separately
  migrated pair/asset account model is required for two independently funded
  directions. Disabling preserves legacy bidirectional operation.
- Accounts with inventory history cannot change output asset or be physically
  deleted. No external funding API, sweeping, custody assertion, or journal-net
  inference was introduced.

## Admin

- Existing Earn product and Convert pair forms configure the policies.
- Decimal amounts remain strings; limits reject negative values, storage
  overflow, malformed input, and effective precision beyond the chosen asset.
  Earn blanks become null and zero roundtrips. Convert funding cannot silently
  become zero on blank input; toggling protection requires an explicit total and
  funding reference, preserving existing consumed funds.
- Unchanged Convert inventory is omitted from a pair edit; changing it submits
  the last loaded inventory revision. Stale revisions are rejected.
- Existing product/pair before/after audits include new fields. Convert adds
  independent funding-history and per-quote allocation evidence.
- Owned resource rows and shared Chinese field labels include these policies.
- Convert explicitly displays that its funding total is a configured budget,
  not verified custody, and that enabled protection refuses reverse output.

## Seconds Slice

- Migration `0138_seconds_payout_capacity.sql` adds nullable
  `open_payout_capacity` to the existing product. Null preserves behavior;
  zero rejects new positive obligations.
- Product configuration, DTOs, audit snapshots, existing Admin form/resource,
  and the admission-only exposure child are wired. No payout calculation,
  settlement/refund money path, or platform journal implementation was changed.
- Under the product lock, admission repeats exact replay before calculating the
  same-product, same-stake-asset sum. Each `opened` or `manual_review` order uses
  its original net-rate snapshot; candidate uses its selected cycle rate.
  The maximum gross amount is principal times one plus that net rate, rounded
  upward to 18 places. No guessed maximum rate or cross-currency sum is used.
- Settled or atomically `refunded` orders no longer reserve unpaid liability.
  Manual review does not release it. Product currency changes are rejected
  while incompatible unpaid obligations remain.
- Euclid's prospective refund helper is called after a genuinely new order
  insert succeeds, before wallet debit, still under the product lock. Missing
  or disabled policy does not snapshot; replay branches never call it. This
  slice only wires the helper, not refund eligibility or money movement.
- A standalone specification now lives at
  `.trellis/spec/backend/aggregate-exposure.md`; neither global index nor the
  concurrently owned seconds specification was edited.

## Verification

- Dedicated local MySQL database: `e06_pool_inventory_test` on port 13316.
  Full migration chain, including 0136, applied there. Applied migration is
  immutable.
- Earn focused route tests: 2/2 passed. Cover 32 concurrent small subscriptions
  with exactly seven admitted, boundary liability, tightened principal cap,
  exact replay when full, two redemptions releasing capacity, disabled limits,
  admin null/zero/exact large decimals, rejection precision and audit.
- The Earn audit rollback fixture formerly used a nonexistent administrator,
  so authentication returned 401 before the expected audit failure. With
  explicit authorization it now uses a real authenticated administrator and
  an action/admin-scoped trigger, following the existing Convert test pattern.
  The trigger is dropped before propagating request errors or assertions.
  No production authentication or audit behavior was changed.
- Final full Earn route regression: 22/22 passed against the dedicated MySQL,
  including that audit rollback and both exposure tests.
- Convert direct transactional MySQL test: 1/1 passed. Covers 32 concurrent
  consumers with exactly seven successes, helper replay, rollback of allocation
  and funding update, stale revision, lowering below consumed, precision,
  disabling without erasure, currency-change refusal, and funding-audit count.
- New Admin exposure form tests: 6/6 passed; existing financial configuration
  tests: 15/15 passed. Earn resource tests: 4/4 passed.
- Web typecheck and `git diff --check` passed.
- Latest exposure forms and Chinese presentation gate: 27/27 passed; both
  previously unrelated missing labels were repaired by their owners.
- Focused rule/wiring tests and the explicit inventory MySQL helper: 7/7
  passed. Includes upward gross obligation rounding, null/zero/precision,
  Earn's original subscription/redemption journal checks and two added
  configuration precision validations.
- Inventory's optional real-DB lib test reads only
  `CONVERT_INVENTORY_DATABASE_URL`, not process-global `DATABASE_URL` mutated
  by unrelated configuration unit tests. It rejects non-loopback hosts,
  sockets and database names without the `_test` suffix. The actual run used
  `mysql://root@127.0.0.1:13316/e06_pool_inventory_test`; missing explicit
  variable is reported as a skip, not database verification.
- Scoped ESLint, Web typecheck and owned Rust formatting passed. The exact
  std-only backend architecture test was also independently compiled with
  `rustc --test` and passed 11/11 while Cargo was shared.
- Removed the two unnecessary `&format!` borrows in Earn exposure fixtures;
  scoped rustfmt and `cargo clippy --test earn_routes -- -D warnings` passed.
  Final `git diff --check` passed. Full Web release gate remains with main;
  no temporary browser source remains in its lint scope.
- Authoritative Convert route test: 1/1 passed. Covers Admin funding config,
  wallet-failure rollback, 24 concurrent confirmations with exactly seven
  committed 20-unit outputs against 140 explicitly configured units, original
  duplicate-confirmation 409, funding decrease refusal, preserved consumption
  on disable, stale revision refusal, two successful funding audits, and a
  real reverse quote whose confirmation is refused while inventory is enabled.
- Seconds capacity route test: 1/1 passed with real MySQL and temporary Redis.
  Covers 32 concurrent small openings with exactly seven admitted at the
  boundary, tightened cap, unchanged old rate snapshots after product edits,
  exact replay when full, manual-review obligations retained, two real
  settlements releasing capacity, incompatible stake-asset change refusal,
  zero/null and invalid limits, and four product-configuration audits.
- The same seconds test exercises prospective policy compatibility through
  the real refund API: missing policy creates zero snapshots; 16 concurrent
  same-key opens create exactly one order/snapshot; enabling never backfills
  old orders; disabling does not revoke that captured policy. An eligible
  manual-review order refunds original principal once, replays its receipt,
  becomes `refunded`, and releases capacity for a new order. Settlement and
  refund implementations remain owned by their respective agents.
- All-target compile passed after initial seconds wiring and main reported
  a later successful run. Intermediate shared OpenAPI/refund test-file
  blockers were resolved by their owners before the successful route rerun.
- Actual Earn/Convert/Seconds Semi forms, real Quill and application CSS were
  rendered in Ego Browser using an isolated local API-response fixture.
  All three passed 1440x1000 desktop and 390x844 narrow checks with zero
  document or dialog horizontal overflow. Screenshots were inspected:
  `/tmp/e06-{earn,convert,seconds}-{desktop,narrow}.png`.
  Long amounts retained all decimal digits in input values, with normal
  horizontal input scrolling; no floating-point conversion was observed.
- Browser interactions confirmed negative caps disable submission; Earn and
  Seconds blank caps submit null; Seconds zero remains valid and its `1.4`
  historical rate remains unchanged in the submitted cycle. Convert's budget
  versus custody and enabled reverse-refusal notices were visible at both
  widths. Disabling submits the exact original funding string, revision and
  explicit reference without erasing consumed history.
- Browser mutation requests were intercepted locally, not sent to a real
  financial backend. Temporary `.e06-browser` entry files were deleted,
  the browser task was finished, local Vite port 5196 was stopped and the
  dedicated temporary Redis on 16339 was shut down after tests.
- Production MySQL 8.4 was not tested; local database execution uses MySQL 9.3.

## Remaining Pool Limitations

- These product/pair policies do not implement a global capital or liquidity
  pool across products, lending, Earn, seconds, or custody venues.
- No unlike currencies are summed or converted using guessed prices.
- Earn caps bound contracted obligations but do not prove cash availability.
  Convert funding references are operator assertions, not verified external
  deposit receipts; funds are not automatically replenished from input flows.
- Existing exposures are not rewritten, liquidated, or reconciled to invented
  balances when a cap is tightened.
- Loan coverage remains documented separately in `e06-loan-delivery.md`.
