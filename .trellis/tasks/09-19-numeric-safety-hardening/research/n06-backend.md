# N06 Backend Numeric Safety

## Ownership And Design

This slice owns Wallet, Convert, Earn, Loan, Seconds, New Coin, Prediction,
Agent, Risk, Quick Recharge, their workers, and Admin business code except
`admin/service/market.rs`. Shared numeric/time/config ingress, transport DTOs,
Spot/Margin and frontend work belong to other implementers. Existing dirty
business changes are retained. No migrations, production operations, commits,
task metadata, PROGRESS or spec indexes are changed by this slice.

Use the main owner's `numeric::ensure_amount_storage` and
`ensure_decimal_storage` at source-storage and final-write boundaries.
Intermediate products/divisions retain arbitrary precision until the existing
business quantization step. Do not round an account snapshot to asset precision
or change existing fee, payout, interest or exposure policies.

## Inventory

| Surface | Findings And Implementation |
| --- | --- |
| Wallet asset precision | Replaced narrowing `i64 as u32` effective scale with saturating conversion; compute effective scale with i128 instead of overflow-prone normalization. Trailing zeros and true zero remain valid. |
| Wallet writes | Shared balance, ledger and per-asset platform journal writers enforce DECIMAL(38,18); generic SQLx account/ledger repository checks every snapshot before opening the transaction. No independent snapshot rounding. Deposit source and withdrawal total/fee checks precede persistence. |
| Wallet reporting/pagination | Read-only aggregate values remain BigDecimal; SQL SUM can legitimately exceed an individual wallet envelope. Page-count conversion to u32 now fails explicitly instead of wrapping. Existing u32 SQL limit/offset widening to i64 is lossless. |
| Convert | Source amount and rate snapshots checked; fee and output checked **after** existing asset quantization. Reverse-rate division is still permitted before explicit rate normalization. Final settlement preserves historical target dust, adds exact quote amount and checks both balances. TTL constructor rejects duration/date overflow and TIMESTAMP storage overflow. |
| Convert configuration | Source/target minima/maxima enforce 38,18; existing [0,1) ratio and eight-place rules retained. New Coin fixed-rate config enforces 38,18. `floating_rate_json` is persisted metadata, but executable rules explicitly require `rate_source=fixed`; no invented floating-rate behavior or generic rejection of inert JSON. |
| Earn | Shared storage validator replaces duplicated scale/integer implementation, accepting insignificant trailing zeros and preserving old service error messages. Existing APR 18,8, fee policy [0,1], amount 38,18, asset quantization and term bound remain. Final redemption/subscription balances and platform legs checked; high-precision yield/fee intermediates remain permitted. Maturity uses shared TIMESTAMP storage validation. Exposure SUM/reservations are comparison values, not prematurely forced into per-row storage. |
| Loan | Source amount precision now also checks 38,18; product and runtime interest rate enforce 18,8. Quantized interest **and principal plus interest** must fit before repayment. Rounded LTV is checked before its persisted snapshot. All four wallet operations and collateral liquidation balance writes check final capacity; ledgers/journal enforce exact storage. Approval reads the database clock after locks and validates maturity via shared checked expiry before binding the same approved/disbursed time and due date. Existing actual-day rounding, collateral recovery and no-external-sale policy unchanged. |
| Seconds | Shared storage validation preserves 18,8 payout and 38,18 stake envelopes with trailing-zero acceptance and old service error messages. Entry-price storage, TIMESTAMP-bounded opening expiry and final wallet/ledger writes guarded; per-asset journal goes through shared Wallet writer. Existing net payout and gross exposure semantics unchanged. Worker uses the same final writes. |
| New Coin | Inputs, authoritative price*quantity result, quantized unlock fee, wallet balances, frozen subscription funds, release balances and ledger checked. Admin distribution/refund balances checked too. Relative unlock duration/date or TIMESTAMP storage overflow returns a domain error instead of panic; admin relative configuration uses the same storage envelope. Narrow fee-rate storage check follows the existing asset-scale error contract. No relaxation of exact price*quantity admissibility. |
| Prediction | Stake/probability, 18,8 config fee, quantized shares/fee/total payment and wallet/ledger storage guarded. Dynamic payout-cap JSON validates before admin write and runtime use; explicit invalid cap cannot silently fall back to unlimited. Upstream decimal parser bounds resource use but preserves high-precision probability intermediates before existing normalization. |
| Agent | Source commission record and quantized payout use 38,18; persisted differential rate uses 18,8; invalid asset precision fails rather than clamps. Cumulative-before-differential quantization unchanged. Admin payout/ledger writer guarded. Huge worker minimum age yields earliest cutoff (no eligible payouts), not panic/future cutoff. |
| Risk | Dynamic decimal strings/numbers bounded to 256 characters and exponent +/-256 before BigDecimal. JSON thresholds are **not** storage amounts: existing high-precision aggregate thresholds above 38,18 remain accepted. Existing u32 technical rules and precedence unchanged. |
| Quick Recharge | Actual schema is DECIMAL(36,18), narrower than Wallet; config, fiat input, callback decimal and provider actual amount use that envelope. Callback amount must fit locked asset precision, never silently round verified payment data. Final wallet balance checks 38,18. Paid replay remains ahead of new wallet/config checks. |
| Admin pair asset checks | Create/update lock asset metadata and enforce qty_precision <= base precision, minimum notional fits quote precision, and both assets have valid 0..18 precision. Create retains active-asset policy; update does not newly require active assets. No existing order admission/replay code changed. Pure helper is in `service/wallet_assets.rs`, avoiding the Spot owner's `service/market.rs`. |
| Chain worker | Gateway amount strings use bounded exact parser before creating deposit requests. Shared Wallet deposit path retains its fee snapshot, net-credit and reorg rules. |

## Changed Files

Production files:

- `src/modules/wallet/domain.rs`
- `src/modules/wallet/infrastructure/{shared,accounts_ledger,deposits,withdrawals}.rs`
- `src/modules/convert/{domain,service,infrastructure}.rs`
- `src/modules/earn/{service,infrastructure}.rs`
- `src/modules/loan/{service,application,infrastructure,liquidation}.rs`
- `src/modules/seconds_contract/{service,application,infrastructure}.rs`
- `src/modules/new_coin/{domain,service,infrastructure}.rs`
- `src/modules/new_coin/infrastructure/{unlock,subscription_freeze}.rs`
- `src/modules/prediction/{service,application,infrastructure}.rs`
- `src/modules/quick_recharge/{service,application,infrastructure}.rs`
- `src/modules/risk/service.rs`
- `src/modules/agent/infrastructure.rs`
- `src/modules/admin/service/{agents,convert,wallet_assets,new_coin}.rs`
- `src/modules/admin/application/market.rs`
- `src/modules/admin/infrastructure/{market,wallet_assets,new_coin,new_coin_settlement}.rs`
- `src/workers/{agent_commission_settlement,wallet_chain}.rs`

Tests:

- `tests/unit_src/src_modules_{wallet_mod,convert_mod,loan,earn_service,new_coin_mod,quick_recharge,admin_service,seconds_contract,risk_mod,prediction}_tests.rs`
- `tests/unit_src/src_workers_agent_commission_settlement_tests.rs`
- `tests/{admin_routes,convert_routes}.rs` (test module declarations only)
- `tests/admin_routes/numeric_safety.rs`
- `tests/convert_routes/numeric_safety.rs`
- `tests/loan_risk.rs`
- `tests/earn_routes.rs`
- `tests/earn_routes/exposure.rs`
- `tests/seconds_contract_routes/exposure.rs`

## Validation Record

- First `cargo check --lib`: found String symbol fields used instead of the
  existing u64 asset-ID fields in new Admin pair checks. Fixed to
  `before.base_asset_id`/`before.quote_asset_id`; no casts or symbol parsing.
- First `cargo test --lib numeric_safety -- --nocapture`: 11/11 passed.
  Later edits add Prediction JSON and Quick Recharge asset precision coverage;
  final scoped regression results will be recorded below.
- First isolated DB launch was denied by sandbox networking; not DB evidence.
  Elevated rerun executed the DB tests: Admin recharge rollback passed; pair
  test's PATCH fixture had create-only fields and correctly got DTO 422.
  Fixture corrected to use the real PATCH schema, retaining the expected
  service-level 400 assertion.
- Owned changed Rust files formatted with rustfmt, edition 2024,
  `skip_children=true` to avoid other owners' files. `git diff --check` passed.

- Scoped units after time changes: 237/237 passed with local mock HTTP bind
  permission. Includes the previously failing
  `new_coin_unlock_fee_rate_must_fit_persisted_precision` unchanged.
  `DATABASE_URL` and `CONVERT_INVENTORY_DATABASE_URL` were unset and the explicit
  inventory DB test was excluded; the existing reconciliation test returns
  early without DATABASE_URL, so this is **not** database evidence.
- Initial full scoped unit attempt: 232/235 passed, with two sandbox mock bind
  failures and the New Coin validation-order regression. Escalation and the
  service-order fix resolved all three; no assertions were weakened.
- Isolated numeric DB tests: Admin 2/2 and Convert 1/1 passed. They check
  locked asset metadata, exact dust retention and transaction rollback.
- First expanded DB run: Convert 20/20, Earn 19/22. Loan risk separately 1/1
  including huge-term rejection with no disbursement/loan wallet, followed
  by normal approval, exact database-day alignment and replay.
- Seconds expanded first run: 26/28, including the actual dedicated-database
  principal-refund test. Earn/Seconds failures were service-message drift
  and expected ingress changes from 400 JSON to 422 deserialization text.
  Restored old narrow service error messages. Updated only affected ingress
  assertions to exact 422, field name and DECIMAL envelope; added unchanged
  persisted configuration/audit and actual Earn wallet/subscription/ledger/
  private-event assertions before successful same-key normal subscription.
- Architecture 11/11 and documentation 1/1 passed. Initial global formatting
  checks found concurrent Margin, later Spot, owner edits; no cross-owner
  files were reformatted. Final scoped format and regression results follow.

### Final Results

| Command / Target | Result | Evidence Boundary |
| --- | --- | --- |
| Scoped lib command below | 237 passed, 0 failed | Actual service/domain/unit mocks executed; optional reconciliation DB branch returned early and explicit inventory DB test was excluded. Not counted as DB evidence. |
| Admin `numeric_safety` selection | 2 passed | Real isolated DB pair create/update rejection, metadata validity and recharge rollback. |
| Full `convert_routes` | 20 passed | All targets ran with DB/Redis configured, including normal/reverse quotes, fees, history authority, concurrency, inventory, replay, dust and overflow rollback. |
| Full `earn_routes` | 22 passed | Normal/early redemption, fees, subscription, same-key replay, concurrent capacity and audit rollback; invalid 19dp/21-digit source requests leave wallet buckets, subscriptions, ledger and events unchanged, then same-key valid request succeeds. |
| Full `loan_risk` | 1 passed | Real approval/repayment/liquidation lifecycle and concurrent terminal handling; huge-term approval rollback, exact database-clock maturity alignment and approval replay. |
| Full `prediction_commission_routes` | 6 passed | Normal order/commission, revision/config invalidation, end-time rejection, win/loss/capped payout and refund journal replay. |
| Full `seconds_contract_routes` | 28 passed | Normal opening, win/loss, concurrency/replay, recovery, exposure and principal refund. Dedicated refund DB branch actually ran. |
| `backend_architecture` / `backend_documentation` | 11 / 1 passed | Final production edits included; no guard exceptions or broad scan allowlists. |
| Scoped rustfmt / final `cargo fmt --all -- --check` / `git diff --check` | Passed | Scoped format covers listed Rust files; final global formatting passed after other owners stabilized. |

The expanded integration command finished with **77 passed, 0 failed, 0
ignored**, plus the two separate Admin numeric tests. This count includes
intentional authentication/no-database validation tests inside those targets;
it is not a claim that all 77 individually perform SQL. Database-enabled test
branches did not return early for missing environment. The standalone first
Convert numeric test is already included in its full target and is not counted
twice. A temporary test compile error used nonexistent subscription `try_recv`;
it was corrected to the existing asynchronous `recv` with a bounded timeout,
then the complete command passed.

No independent all-target check or full Clippy claim is made by N06; the main
owner coordinates those workspace-wide gates. No production code changed after
the final scoped unit/gate run; the final test-only event assertion was included
in the successful full integration run.

### Reproducible Commands

Local isolated MySQL is 9.3, not production 8.4. No services were stopped.
The refund fixture requires its pre-existing exact dedicated database name;
its host/database safety assertions were not relaxed.

```sh
env -u DATABASE_URL -u CONVERT_INVENTORY_DATABASE_URL \
  -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY \
  -u http_proxy -u https_proxy -u all_proxy \
  cargo test --lib -- wallet convert earn loan seconds_contract new_coin \
  prediction agent risk quick_recharge admin::service \
  --skip explicit_inventory_concurrency_rollback_replay_and_revision

env DATABASE_URL=mysql://root@127.0.0.1:13316/numeric_backend_test \
  REDIS_URL=redis://127.0.0.1:16386/4 \
  cargo test --test admin_routes --test convert_routes numeric_safety \
  -- --nocapture --test-threads=1

env DATABASE_URL=mysql://root@127.0.0.1:13316/numeric_backend_test \
  REDIS_URL=redis://127.0.0.1:16386/4 \
  SECONDS_REFUND_TEST_DATABASE_URL=mysql://root@127.0.0.1:13316/hardening_commission_test \
  SECONDS_REFUND_TEST_REDIS_URL=redis://127.0.0.1:16386/4 \
  cargo test --test convert_routes --test earn_routes --test loan_risk \
  --test prediction_commission_routes --test seconds_contract_routes \
  -- --nocapture --test-threads=1

cargo test --test backend_architecture --test backend_documentation
cargo fmt --all -- --check
git diff --check
```

## Verification Limits

Unit tests exercise real Rust functions, not reimplemented arithmetic.
New isolated DB tests exercise real routes, wallet rows, order/quote state,
ledger and journal counts and audit/receipt rollback.

Even with those focused tests passing, they do not prove every business path:
Earn manual/auto-redemption overflow, Loan approval/repayment/liquidation
overflow, Seconds manual/worker overflow, New Coin allocation/unlock overflow,
Prediction terminal/refund overflow, Agent commission overflow, and malformed
chain-provider/callback end-to-end paths require their own DB scenarios.
Existing narrower business regressions may execute separately; their exact
commands and results must not be generalized into coverage of these new edge
cases. No production balances, historical migrations, external providers or
MySQL 8.4 have been tested here.

Normal Earn/Loan/Seconds/Prediction lifecycle coverage above is real DB
evidence, but does not substitute for new maximum-capacity arithmetic fixtures
on every terminal path. Loan **time** overflow is specifically tested; Loan
**monetary** terminal overflow remains in that unverified list. No broad
Wallet/New Coin/Agent/Quick Recharge provider-worker DB suite was run in this
slice.

Some worker/read-model date calculations add small bounded constants to trusted
DB timestamps; this slice fixes unbounded duration entry points, not every
theoretical DateTime::MAX construction. Legacy product/asset precision changes
can still render **new** operations inadmissible, but successful original
replay branches are not moved behind new mutable-configuration checks.

### Retained Non-Authoritative / Bounded Surfaces

- Reporting aggregates remain decimal and may exceed an individual row's
  storage capacity; no reporting result becomes a wallet source amount.
- Existing u32 paging limits/offsets widen losslessly to SQL i64. Precision
  constants passed to private Earn/Seconds validators are fixed at 8 or 18;
  no request-provided scale controls their casts.
- The Redis-only Convert adapter receives its positive signed TTL from the
  validated quote constructor; this slice did not change its public cache
  entry shape. A caller fabricating an entry without that constructor remains
  outside the validated use-case contract.
- Wallet historical-return Mongo close parsing is a read-only reporting
  boundary and was not hardened here. Main owns source market ingestion;
  damaged legacy local market records are not proved safe by these write
  guards.
- Existing cycle-index casts and retry-delay SQL boundaries were surveyed but
  not comprehensively replaced. Normal configured requests have bounded
  duration/transport size; direct infrastructure invocation with fabricated
  huge vectors/delays and dates near the TIMESTAMP ceiling needs separate
  coverage. This inventory is a limitation record, not a scan allowlist.
