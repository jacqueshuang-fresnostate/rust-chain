# N04 Spot Numeric Safety

## Decision And Tracking

- [x] Inventory creation, manual fills, triggered fills, cancellation, wallet
  writes, pair configuration, identifiers and paging.
- [x] Generated quote amounts truncate toward zero per actual execution to the
  quote asset precision. Source price/quantity never round; quantities also
  satisfy the actual base asset precision. A zero quote execution is rejected.
- [x] Buy reserve is floor(limit/reference * total quantity). Each fill uses
  floor(execution price * slice quantity), shared by debit/credit, commission
  basis and journal. Partial fills retain surplus; full fill/cancel release
  the stored reserve minus actual debit/release evidence.
- [x] Historical freeze ledger evidence takes precedence over reservation
  snapshots. Actual frozen settlement ledger debits replace SQL recomputation
  of price * quantity. No whole-wallet fallback, migration, balance rewrite,
  independent wallet-bucket rounding or re-quantization of historical refunds.
- [x] Real MySQL regressions and final delivery inventory.

Main accepted the design. N06 owns pair create/update transaction checks against
asset metadata; N04 owns pure pair bounds in admin service. Main owns monetary
DTO deserializers in spot presentation and shared storage guards. Do not stop
the shared local MySQL/Redis services.

## Verification So Far

- `cargo test --lib spot_numeric -- --nocapture`: 2/2 pure tests passed.
- `cargo test --lib --test spot_routes spot_numeric -- --nocapture` against
  the isolated schema: 2/2 pure and 3/3 real database tests passed (not skipped).
  The initial sandbox connection refusal required an approved unsandboxed
  rerun. One first-run test-helper failure on plaintext DTO rejection was fixed;
  it was not a financial-path failure.
- Isolated DB assigned: local MySQL 9.3 port 13316, schema numeric_spot_test;
  Redis port 16386 DB 2. Production MySQL 8.4 has not been exercised.

## Inventory

| Surface | Boundary |
| --- | --- |
| Domain order validation | Storage 38,18; pair precision 0..18; no narrowing scale cast |
| Order creation/fingerprint | Source values bounded before canonical expansion |
| Reservation | Actual base/quote metadata in transaction; shared quote quantization |
| Manual/automatic/liquidity fill | Same quote result across all monetary legs |
| Remaining reservation | Stored freeze, debit and release evidence scoped to user/asset/order |
| Cancel and full-fill surplus | Exact historical remainder, no current-precision rewrite |
| Wallet/ledger writes | Amount and every resulting bucket fit storage before writes |
| Pair admin service | Precision range 0..18 and minimum storage capacity |
| Cached price/time | Bounded decimal parser, checked age, future/stale rejection |
| IDs and paging | Positive string u64 business IDs; bounded u32 limit/offset with widening conversions |
| Generic SpotService | Explicit actual precision and remaining-reserve inputs, shared quote truncation, exact final surplus |
| Standalone SQL writes | Source price/base quantity checks and storage-safe fee/order columns |
| Read-only average price | SQL weighted-average arithmetic is display-only, never reserve/debit authority |
| Raw domain products | Minimum-order comparison and legacy request identity only; not wallet write amounts |

## Follow-Up During Implementation

The 3 real database regressions cover historical NULL/zero snapshots with
freeze evidence, partial quantities missing debit evidence, stored dust beyond
current precision, preservation of unrelated frozen funds, 8-way fill replay,
8-way cancellation replay, last-credit overflow rollback, partial cancel,
three fills with final dust release, both automatic inventory directions,
20-fractional-digit raw products, zero quote rejection, per-asset zero-sum
journals and exact matching quote debits/credits.

Existing seed helpers now explicitly persist reservation snapshots; the
missing-evidence regression remains separate and expects failure.

Legacy limit/stop-limit replay compares original source parameters rather than
derived reservation arithmetic. Historical market orders with a stored request
reference compare that source snapshot; missing reference retains the existing
limited reserve-based evidence check. No historical fingerprint is fabricated.

The full serial integration regression and scoped formatting checks passed;
final focused library verification is recorded below.

## Full-Suite Diagnostic

- First 4-thread full run: architecture 11/11, documentation 1/1, domain 9/9;
  Spot routes 63/65. One original negative test still expected HTTP 400 for a
  zero reservation without evidence; it now explicitly asserts HTTP 409 and
  the missing-freeze-evidence reason. No rejection was weakened.
- Second 4-thread run: original Spot routes 62/62 and two numeric regressions
  passed; only the auto numeric fixture failed (total 64/65).
- Captured MySQL error 1213 / SQLSTATE 40001. `SHOW ENGINE INNODB STATUS`
  identified existing cross-pair account-bootstrap lock inversion: transaction A
  holds `users.PRIMARY` supremum and waits to insert into `wallet_accounts`;
  transaction B holds `wallet_accounts.PRIMARY` supremum while upserting the
  singleton system liquidity user and waits for `users`. The deadlock graph
  does not involve the new pair/asset precision locks.
- The batch's existing behavior logs and rolls back the victim, keeps the
  order pending and returns zero executions. The failed fixture was inspected:
  original reserve 1, user quote available 9/frozen 1, user base available 10,
  both system inventories 100, no partial financial commit.
- Standalone automatic regression passed all four scenarios, including real
  buy/sell `1e-18` quote execution at precision 18 and legacy no-fingerprint
  replay with non-recomputed reservation semantics.
- No blanket retry, auto-rounding workaround, changed isolation level or
  production bootstrap change was added. Final full suite is run serially to
  isolate global fixture resources; explicit intra-test concurrency remains.
  Cross-pair account bootstrap deadlock/liveness remains a separate follow-up.

## Reservation Evidence Contract

- A positive persisted freeze ledger sum scoped to user, asset and order
  overrides the reservation snapshot. Without that ledger, an existing positive
  snapshot remains valid evidence; new normal test fixtures explicitly seed this
  snapshot rather than masquerading as historical rows without evidence.
- NULL/zero snapshots are supported when actual freeze evidence exists.
  No positive evidence returns HTTP 409; tests preserve this negative assertion
  rather than assigning the whole wallet frozen balance to the order.
- Partial sells require stored base debits equal to filled quantity. Partial
  buys require matching base credits and debit/credit counts, plus a positive
  quote debit. The current trade placeholder is not evidence of money movement.
- Remaining funds are original stored freeze minus actual debit/release rows.
  Order and wallet locks serialize fill/cancel/replay; eight-way replay tests
  assert exactly one financial effect. Historical release checks storage
  capacity only and preserves dust beyond the asset's current precision.
- Legacy request fingerprints are not invented. Existing source snapshots
  retain their prior replay meaning; lack of new request metadata alone does
  not invalidate a legitimate old limit order.

## Final Validation

On the isolated MySQL 9.3 schema `numeric_spot_test` and Redis DB 2:

```sh
env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY \
  DATABASE_URL=mysql://root@127.0.0.1:13316/numeric_spot_test \
  REDIS_URL=redis://127.0.0.1:16386/2 \
  cargo test --test spot_routes --test spot_domain \
  --test wallet_spot_services --test wallet_spot_sqlx_repositories \
  --test backend_architecture --test backend_documentation \
  -- --nocapture --test-threads=1
```

- Spot routes **65/65**, including all original **62/62** and three new numeric
  regressions; no missing-environment skip.
- Spot domain **9/9**, wallet/spot service **6/6**, SQL repositories **2/2**.
- Architecture **11/11**, executable documentation **1/1**.
- `cargo test --lib spot -- --nocapture`: **31/31** selected library tests
  passed, including source/generated amounts, partial-fill conservation,
  trading-pair configuration and exact ID boundaries.
- Scoped `rustfmt --edition 2024 --check --config skip_children=true` and
  `git diff --check` passed. The three requested `cmp_owned` findings are fixed;
  Main owns the integrated strict Clippy rerun.
- The four-thread result remains **64/65**, not a passing parallel gate. The
  automatic regression passes independently and serially, including real
  `1e-18` executions for both sides with quote precision 18.

## Delivery Files And Boundaries

- `src/modules/spot/{domain,service}.rs`.
- `src/modules/spot/application/{order_creation,settlement,triggering}.rs`.
- `src/modules/spot/infrastructure/{common,market_prices,order_repository,read_models,trade_settlement,wallet_accounts}.rs`.
- `src/modules/admin/service/market.rs`: pure trading-pair configuration bounds
  only; N06 owns transactional checks against asset configuration.
- `tests/{spot_domain,spot_routes,wallet_spot_services}.rs`,
  `tests/spot/numeric.rs`,
  `tests/unit_src/src_modules_spot_service_tests.rs`.
- `.trellis/spec/backend/spot-orders.md` and this delivery record.

This is the N04 delivery inventory, not every dirty file versus HEAD. Earlier
Spot hardening changes and Main's presentation deserializers remain intact.
Main owns the aggregate PROGRESS entry and final task metadata.

No new migration, historical backfill, independent bucket rounding, blanket
test retry, production operation, commit or service shutdown was performed.
MySQL 8.4 production compatibility is not claimed from a MySQL 9.3 test.
The known account-bootstrap lock inversion remains a liveness risk: a victim
transaction rolls back without partial funds movement, while the batch logs
and skips that attempt. Later execution depends on another market invocation;
this delivery does not guarantee automatic immediate retry.
