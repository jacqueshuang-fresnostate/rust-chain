# Seconds contract net payout-rate correction

## Goal

Normalize the seconds-contract product configuration so the configured payout
rate is a net profit ratio that excludes principal. A configured `0.4` must be
shown as 40% net profit and a 100-unit winning stake must settle to 140 units in
total. The currently observed gross-style values (`1.4`, `1.5`, `1.6`, `1.8`)
must be corrected in product configuration without rewriting historical order
snapshots.

## Confirmed requirements

- Correct the affected seconds-contract product and cycle configuration from
gross total-return factors to net profit rates: 1.4 -> 0.4, 1.5 -> 0.5,
1.6 -> 0.6, and 1.8 -> 0.8.
- Apply the correction with a new immutable SQLx migration; never edit an
already-applied migration.
- Update product master rows and cycle rows used for new orders. Leave
`seconds_contract_orders.payout_rate` snapshots unchanged so old orders retain
the rate that was actually recorded when they were opened.
- Keep the existing settlement formula (`stake + stake * payout_rate`) and the
existing decimal precision/validation rules. Do not subtract one in mobile or
otherwise hide a data-contract mismatch at presentation time.
- Make the net-rate unit explicit in the mobile confirmation/active-order copy
and in the Admin cycle editor hint, while preserving the decimal API contract
and exact arithmetic.
- Add regression coverage for the migration guard/mapping, a new 0.4 order and
140 total settlement, and a historical 1.4 order snapshot remaining unchanged.

## Acceptance criteria

- [x] A fresh database applying all migrations converts only the explicitly
identified gross configuration values in seconds product/cycle rows.
- [x] Reapplying the correction SQL is a no-op and the SQL never updates the
seconds order snapshot table.
- [x] New product API data and newly opened orders expose `0.4` for the 40%
configuration; a 100-unit win returns 140 total.
- [x] Existing orders with a stored `1.4` snapshot still expose `1.4` and use
that snapshot for their historical settlement/audit calculations.
- [x] Mobile shows the configured net rate (0.4 -> 40%) and labels it as
excluding principal; no client-side `-1` transformation is present.
- [x] Admin operators see the same net-rate explanation next to cycle inputs.
- [x] Focused Rust, mobile, and Admin checks pass; changed files are formatted
and `git diff --check` is clean.

## Definition of done

- Tests added or updated at the nearest backend/mobile/Admin boundaries.
- SQL migration is reviewed for idempotence, rollback/audit implications, and
preservation of immutable order evidence.
- Applicable Trellis specs and `docs/superpowers/PROGRESS.md` record the
contract and verification results.
- No online configuration, order, wallet, deployment, commit, or push action is
performed as part of local implementation unless separately requested.

## Technical approach

1. Add `migrations/0125_seconds_contract_net_payout_rates.sql` as a guarded data
migration. It maps the confirmed gross defaults to their net equivalents in
`seconds_contract_products` and `seconds_contract_product_cycles`, using stable
business identifiers/explicit values and leaving orders untouched.
2. Keep the backend service/application settlement and snapshot code unchanged;
add focused tests around the existing net-rate formula and migration contract.
3. Add symmetric mobile locale wording and an Admin editor hint so operators
enter `0.4` for 40%, without changing request payloads or arithmetic.
4. Run the nearest tests plus required package quality gates and document any
environment-limited checks honestly.

## Decision (ADR-lite)

**Context:** The public product metadata returned 1.4/1.5/1.6/1.8 while the
backend contract defines `payout_rate` as net profit and separately adds
principal on a win. Treating the value as gross in the mobile UI would make
new and old orders financially inconsistent.

**Decision:** Correct the persisted product/cycle configuration to net values
with a follow-up migration, preserve immutable order snapshots, and clarify the
unit at input/display boundaries. Keep settlement semantics unchanged.

**Consequences:** New orders use the intended 40/50/60/80% net rates and pay
140/150/160/180% total respectively. Historical orders remain auditable even if
they carry the old 1.4-style snapshot. Future operator input is less likely to
repeat the error; a broader rate policy or automatic conversion of arbitrary
custom values remains out of scope.

## Out of scope

- Rewriting existing order snapshots, wallet ledger entries, settlements, or
refunds.
- Changing the API unit, settlement formula, product eligibility, min/max stake,
or payout-rate upper-bound policy.
- Mutating the supplied online Admin instance or running live orders.
- Replacing the backend contract with a gross-return representation.

## Technical notes

- Product/cycle schema: `migrations/0021_seconds_contracts.sql` and
`migrations/0066_seconds_contract_product_cycles.sql`.
- Snapshot and settlement paths: `src/modules/seconds_contract/{application,
service,infrastructure}.rs`.
- Mobile financial formatting: `mobile/src/core/secondsFinancial.ts` and
`mobile/src/views/SecondsView.vue`.
- Admin cycle editor and summary: `web/src/admin/resources/actions/secondsContract.tsx`
and `web/src/admin/resources/resourceConfigs.tsx`.
- Evidence and read-only live response are recorded in
`research/payout-rate-evidence.md`; no live write was made.
