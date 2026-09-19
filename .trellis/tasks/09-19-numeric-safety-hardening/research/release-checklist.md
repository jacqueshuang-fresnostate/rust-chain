# Numeric Safety Release Checklist

## Deployment

- Apply existing preceding project migrations in order, then new
  `0141_margin_interest_remainder.sql` before deploying the updated API and
  margin workers. Retire old interest writers together; mixed-version writers
  do not preserve the new carry.
- Rehearse on the production MySQL 8.4 version before release. Local isolated
  MySQL 9.3 success is not certification of a production migration.
- No additional precision configuration is activated automatically. Existing
  asset precision, fee rates, limits, rounding policies and worker schedules
  remain authoritative. New validation can reject a previously accepted
  out-of-range configuration or source amount.
- Monetary payloads remain decimal strings. Numeric IDs above the JS safe
  range are rejected instead of rounded; this release does not migrate the
  whole API to string IDs.
- Request decimal rejection may now occur during DTO deserialization (422)
  instead of later business validation (400). Clients must present the
  rejection and retain user input, not retry with rounded data.
- Current TIMESTAMP columns retain their storage horizon; new generated
  deadlines outside it are rejected. A post-2038 schema extension is a
  separate coordinated migration.

## Historical State

- Do not rescale wallet buckets, edit old balances, rebuild old journals or
  backcharge missing historical interest fractions.
- Interest remainder starts at zero on existing rows and is carried only
  prospectively from the already recorded checkpoint.
- Spot cancellation/full-fill release uses stored reservation and actual
  order-scoped ledger evidence. Missing or inconsistent evidence fails closed
  for investigation, never falls back to all frozen funds in a wallet.
- Account balance overflow must abort the enclosing transaction; no partial
  order, ledger, journal or idempotency-success record may survive.
- Source over-precision is rejected. Intentional authoritative quote
  normalization is preserved and must use the same reviewed quote at execution.

## Separate Operational Finding

Concurrent creation of the singleton spot-liquidity user and its first wallet
accounts can deadlock across pairs (native MySQL 1213/40001). The four-thread
regression captured opposite users/wallet_accounts bootstrap lock order. The
failed transaction rolled back with the order still pending and reservation
unchanged; both automatic directions at 1e-18 passed standalone and in the
serial full suite. This pre-existing account-bootstrap liveness issue is not
fixed by the numeric patch and is not hidden behind test retries. Address lock
ordering in a separately scoped concurrency change.

## Verification Record

- Shared backend tests and field inventory: `n01-shared.md`.
- Mobile decimal, identity, time and UI evidence: `n02-mobile.md`.
- Admin/Agent forms, sums, exports and UI evidence: `n03-admin.md`.
- Spot full lifecycle and historical evidence: `n04-spot.md`.
- Margin/interest migration and settlement: `n05-margin.md`.
- Other backend domains and explicit test limitations: `n06-backend.md`.
- PC compatibility: type-check and 110 existing tests passed. PC financial
  UI arithmetic itself is not part of this implementation scope.

## Final Joint Checks

- Rust all-target tests passed after the final changes, including 501 library
  tests and the 160-field decimal DTO/config guard. The all-target run had
  database environments unset; optional branches are not database evidence.
  Dedicated real financial DB suites are recorded in N04-N06.
- Strict Clippy (`--all-targets --all-features -- -D warnings`), global rustfmt,
  diff check and source integrity (including 16 scanner tests) passed.
- Mobile final release gate: 785/785, type checks, PWA/Tauri-mode builds,
  artifacts and all budgets passed. Browser evidence contains 24 checks across
  eight surfaces and 320/390/1440px; these are local mocks, not native devices.
- Admin/Agent final independent-review gate: 899/899, type/lint, policy15,
  coverage23, build and bundle budget passed. Three confirmed prefill issues
  were reproduced and fixed; see N07 for exact-string and missing-value rules.
- PC compatibility: type check and 110 tests passed; no PC precision rewrite.
- Task-owned MySQL socket and Redis16386 shut down successfully, with PID
  files removed. Frontend verification servers3037/5197 and browser contexts
  were closed by their owners. Original preview5178 still returns HTTP200.
- No commit, push, production deployment, historical backfill or policy
  activation was performed. Task metadata is complete without running an
  auto-commit archive hook.
