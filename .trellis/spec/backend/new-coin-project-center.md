# Admin New-Coin Project Center

## Scope

Admin-only reads/configuration, preserving public and historical settlement
contracts. Routes remain transport-only; application coordinates transactions,
and project-center persistence helpers live in Admin infrastructure.

## Authoritative Read

`GET /admin/api/v1/new-coins/:id` returns the complete project and
`configuration_version`, `subscription_count`, `pending_manual_count`,
`issuance_editable`, `next_lifecycle_status`, `lifecycle_block_reason`.
Read project/counts from one transaction snapshot. Missing IDs return 404.
Project listing filters symbol (substring), lifecycle and operational status
use identical bound predicates for rows and total; pagination is unchanged.

## Issuance Mutation

`PATCH /admin/api/v1/new-coins/:id/issuance` requires a reason, positive
`total_supply`/`issue_price` and original `expected_total_supply`/
`expected_issue_price`. Lock the project before evaluating capability and
original values. Only active preheat projects with no subscriptions, zero
reserved/allocated supply and remaining equal to total qualify.

Check base-asset precision for supply and quote-asset precision for price,
matching creation. Acquire asset locks in stable ID order. Update total and
remaining together; preserve asset identities. Record before/after audit and
event in the same transaction. Late/stale writes return 409 without side effects.

## Configuration Concurrency

Lifecycle, unlock, fee and post-listing-purchase commands accept optional
`expected_config` for compatibility with older clients. New clients send the
opaque original `configuration_version`; compare it under the project lock.
It currently serializes the canonical project audit snapshot, including supply
and linked pair status, so intervening business changes may require refresh.
This is a compare-and-set guard, not a secret or an authorization credential.
Issuance uses its narrower mandatory original-value guard.

## Lifecycle and History

Only preheat -> subscription -> distribution -> listed is offered. Readiness
counts pending manual orders or outstanding manual freeze; listing checks the
same obligations again inside its write transaction. Overview hints do not
replace command validation. Config changes affect future lock snapshots only.
Planned/actual listing and new allocation gates follow the contract below.
No pause/cancellation semantics or retroactive position migration is introduced.

## Authorization and Verification

Both new endpoints use existing Admin authentication and `new_coin.projects`
resource read/write middleware. Frontend visibility is not authorization.
Regression: `admin_new_coin_project_center_guards_issuance_and_stale_configuration`
and project-summary assertions in
`admin_new_coin_manual_distribution_settles_and_refunds_exactly_once`.
Run real local MySQL Admin/user new-coin tests, architecture tests, format,
all-target checks and Clippy; absent-DB skips are not financial test evidence.


## Planned Time, Actual Event and Immutable Lock Gates

- `listed_at` remains the compatible configured **planned** time. Admin also
  returns nullable `actual_listed_at` in Unix milliseconds. Only project creation
  in `listed` and the distribution -> listed command set it, using server time.
  Lifecycle PATCH rejects an explicit non-null `listed_at`; edit plans through
  unlock configuration. Config writes never change the actual event timestamp.
- Migration 0123 recovers an actual timestamp only from an existing recorded
  lifecycle transition/create event. A legacy listed project with no event keeps
  null actual time; neither its plan nor migration execution time is evidence.
- Only NEW prelisting `immediate_on_listing` allocations write a non-null
  `asset_lock_positions.listing_project_id`. Their merge key includes project,
  user and asset. The stored `unlock_at` is source provenance, not maturity;
  release requires that pinned project to be listed with an actual event <= now.
  Changes to a planned date, unlock type or project operational status never
  detach that gate or rewrite amounts/fee snapshots.
- Historical locks keep a null gate and their original timestamps, including
  historical immediate-on-listing, fixed-time and relative-period contracts.
  New fixed/relative locks also use independent maturity, not a listing gate.
  No prior release is reversed. New listed purchases under immediate-on-listing
  are directly available even when their configured plan lies in the future.
  New Admin grants remain distribution-only, not an extra listed purchase path.
- Admin lock list adds `listing_project_id` and `actual_listing_at`. Its
  `unlock_at` is effective maturity (null while gated awaiting listing), so table,
  detail and CSV never advertise a source timestamp as a due unlock time.
  This is a read projection; persisted snapshots remain untouched.
- An audit/event failure during the listing command rolls back both stage and
  actual time. A plan edit or timer alone never advances a lifecycle stage.

## Shared Manual and Worker Unlock Transaction

`unlock_eligibility.rs` owns identity, maturity and fee evidence predicates;
`unlock_scan.rs` uses them for bounded candidates and a separate blocked-fee
count. The worker invokes the same locked release transaction as the user API,
with an explicit clock only for worker/test scheduling.

Release requires pending record, matching owner/asset, positive quantity, active
position and sufficient remaining amount. Fee-disabled historical records pass;
fee-enabled zero amounts require `not_required` plus an asset; positive fees
require `paid`, paid time, the exact negative available wallet ledger and both
matching platform journal legs. A status label alone never proves payment.

Lock order is asset -> wallet -> unlock/position. Precision, wallet locked amount
and predicates are rechecked under lock. Moving locked to available, preserving
frozen, position quantities, terminal unlock status and paired release ledgers
commit together. A repeated terminal record produces no second movement/event.
The worker does not collect fees; it broadcasts only after a successful commit.

Required real tests: `tests/unlock_scanner.rs` (fee evidence, races, rollback),
`tests/new_coin_listing_migration.rs` (exact migration and legacy preservation),
`admin_new_coin_actual_listing_gates_new_locks_without_rewriting_history`
(partial refund, merged gates, rule edits, listing rollback, fixed-time history,
actual event, user/worker race and listed purchase with a future plan).


## Coordinated Rollout

Old release APIs/workers ignore `listing_project_id`. Suspend new-coin writes and
old scanners while applying 0123 and switching API/worker/Admin; never mix an old
release path with newly gated holdings. Rollback versions must retain the gate.
Historical invalid paid evidence is an audit/reconciliation issue, not permission
to fabricate settlement legs, automatically charge again or release early.
