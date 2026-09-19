# E06 Loan Exposure Delivery

## Scope and Policy

- Migration: `0133_loan_exposure_limits.sql`.
- Product fields: nullable `user_principal_limit` and
  `product_principal_capacity`, and `deny_borrowing_while_overdue=false`.
- Null disables the corresponding limit; zero prevents additional borrowing.
  No monetary threshold is chosen by the implementation.
- The applying product's user limit compares the user's principal across all
  loan products **in that product's asset**. Different products may explicitly
  configure different admission policies; this is not a global user credit
  rating or a multi-asset converted credit line.
- Product capacity counts principal for that product and asset only.
  `pending` reserves capacity; `disbursed` and `overdue` retain it.
  `cancelled`, `rejected`, `repaid`, and `liquidated` release it.
- Approval uses current policy and excludes the candidate ID before adding its
  principal once. Lowering a cap does not cancel old orders, rewrite rates,
  accrue additional fees, or change collateral/approval snapshots.
- Overdue denial is opt-in. It checks any loan asset for explicit `overdue` or
  `disbursed` with elapsed `due_at`, so worker lag cannot bypass it. It performs
  no cross-currency addition. Exact create/approve replays bypass new admission.
- A product cannot change asset while pending or outstanding orders remain.
  This prevents applying a numeric limit in a different unit to an old order.

## Transaction and Lock Contract

1. Loan-local user lock row (`loan_user_exposure_locks`, deliberately no FK).
2. Product row (`FOR UPDATE`, without asset join).
3. Candidate order for approval; creation's idempotency read is non-locking.
4. Existing ascending asset and wallet locks.

Creation and approval establish their first consistent-read snapshot only
after acquiring both admission locks. User-wide same-asset and product-wide
increases are therefore serialized and see committed predecessor reservations,
including MySQL's default repeatable-read isolation.

Aggregates do not lock other users' orders. Repayment, rejection, cancellation,
and health liquidation retain their existing order/asset/wallet sequence and
never acquire the new guard. Concurrent terminal releases can make a decision
temporarily conservative but cannot cause excess approval. The overdue query
also checks actual maturity, rather than depending solely on worker status.

No wallet cash, external funding, sweeping, custody balance, or journal-net
inventory is created or inferred. This is a principal exposure budget.

## Admin and DTO

- Existing Loan product form edits and roundtrips the two exact decimal fields
  and the overdue switch. Blank maps to null; zero remains zero.
- Validation uses authoritative asset precision and DECIMAL(38,18) integer
  width. It rejects malformed, negative, excess-precision, and overflow values
  without Number conversion or rounding.
- Malformed/missing policy responses disable editing rather than silently
  clearing protections. The existing status action remains independent.
- Editing a collateralized product preserves its original LTV and oracle
  whitelist fields while editing exposure; this form does not invent new
  collateral rules.
- Product rows expose `reserved_principal` and `outstanding_principal` with
  configuration. These are loaded principal snapshots, not liquid cash.
- Create/update audit snapshots include all three policy values; revision and
  reason behavior is unchanged.
- `src/openapi/loan.rs` is a standalone tested `LoanApiDoc`. The shared
  `src/openapi.rs` is outside this agent's ownership. The main session has
  registered `mod loan;` and merged `loan::LoanApiDoc::openapi()` into both
  Swagger documents. The main session owns the public route integration test.

## Verification Evidence

Completed:

- Real MySQL at `127.0.0.1:13316`, dedicated database
  `e06_loan_exposure_test`, full migration runner invoked on each test setup.
- Four new MySQL route tests passed:
  configuration/defaults/null/zero/storage and precision/audit validation;
  exact boundary, cross-product same-asset and unlike-asset isolation;
  cancelled/rejected/repaid/liquidated status release and approval double-count
  protection; tightened caps; overdue opt-in/defaults and elapsed maturity;
  idempotency and a concurrent approval/repay race.
- 30 concurrent small requests by one user across two products accepted
  exactly 5 at a 0.05 cap; 32 requests by four users accepted exactly 7 at a
  0.07 product cap. Tightening to 0.06 rejected every approval until one pending
  order was rejected; the remaining six approved concurrently with exactly six
  disbursement ledger entries and 0.06 outstanding principal.
- Liquidated release is a status-accounting fixture, not a newly simulated
  external liquidation or proof of recovered cash.
- Full Loan routes: 10/10 passed with
  `DATABASE_URL=mysql://root@127.0.0.1:13316/e06_loan_exposure_test cargo test --test loan_routes -- --nocapture --test-threads=1`.
  The sequential outer test runner avoids the existing filter fixture's
  global row-count assertions racing fixture creation; concurrency inside the
  new tests remains real and unchanged.
- Loan OpenAPI child schema test passed.
- Loan unit tests: 15/15 passed (`cargo test --lib modules::loan::tests -- --nocapture`).
- Existing collateral risk regression: 1/1 passed against a second disposable
  database `e06_loan_risk_test` and temporary nonpersistent Redis on port 16339.
  This includes real authoritative oracle checks, financial rollback,
  idempotency, and health-liquidation versus repayment single-terminal races.
- Admin Loan action tests: 8/8 passed, using real Semi controls.
- Admin typecheck and targeted loan/resource ESLint passed.
- Backend architecture: 11/11 passed.
- `git diff --check` passed.

Initial full route run exposed existing fixtures
that violated current collateral LTV checks, omitted global Admin auth setup,
or did not clean platform journal FK rows. Those fixture-only corrections are
included in `tests/loan_routes.rs`.

Shared resource tests: 83/84 passed; the unrelated margin liquidation test
`opens margin liquidation record details from hidden row IDs` fails because
`getByText('-')` matches two cells. It is left unchanged.

Whole-repository formatting was temporarily blocked by concurrent wallet
module creation and formatting differences in seconds/wallet tests. No
unrelated source was reformatted.

## Remaining E06 Work

- Loan exposure caps are not an explicitly funded lending pool and do not
  certify external custody or liquidity. No loan funding counter was invented.
- Earn principal/yield liability budgets, explicit Convert output inventory,
  withdrawal cumulative budgets, spot net-position and replenishment alerts,
  seconds-contract payout obligations, and external hedging remain separate
  scopes. Do not mark all of E06 complete from this Loan delivery.
- The next authorized slice is Earn liability/pool capacity and explicit,
  audited Convert inventory; migration 0136 is reserved by the main session.
- No production, deployment, commit, push, PROGRESS or global spec-index edits
  were performed by this slice.
