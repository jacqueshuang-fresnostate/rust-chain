# Listing events and unified unlock settlement

## Decision and compatibility

User approved the next lifecycle slice. Keep `listed_at` as the existing planned
listing configuration (UI will label it explicitly), add `actual_listed_at` owned
by server lifecycle/create commands. Do not infer historic actual times from plans;
legacy listed projects without an auditable event retain null actual time.
Actual listing is a stage command, not automatic timer advancement.

For newly allocated immediate-on-listing holdings, pin an explicit project gate
on the lock. A planned timestamp, even past, is never evidence of actual listing.
The gate survives later rule edits. New allocations after listed are available.
Existing lock snapshots/gates are untouched; fixed/relative locks retain their
current independent maturity semantics. No automatic fee deduction, relocking,
withdrawal restriction, new pause/cancellation, or production mutation is added.

## Implementation sequence

1. Share manual/worker release transaction, fee evidence predicate, lock order,
   precision checks and replay behavior. Candidate query cannot hide invalid
   paid/not_required evidence by relying on a label.
2. Add immutable listing-event / new-lock-gate migration, server-owned actual
   event timestamp, and gate-aware allocation/maturity. Keep historical data intact.
3. Label planned/actual times and waiting-for-listing lock records in Admin;
   preserve public compatibility, category forms and exact timestamp values.
4. Real MySQL: malformed paid evidence, zero fees, concurrent manual/worker
   release, rollback, past/future plans, actual listing and historic fixed/relative
   behavior. Run Rust and Web gates and update executable specs/progress.

## Evidence

- `src/modules/new_coin/infrastructure/unlock.rs`: manual release checks ledger
  and both platform journal legs after locking asset/wallet and unlock/position.
- `src/workers/unlock_scanner.rs`: separate mutation takes opposite lock order
  and accepts paid/not_required labels without validating settlement evidence.
- `src/modules/admin/application/new_coin.rs`: lifecycle currently overwrites
  `listed_at`; unlock configuration also writes the same field.
- `src/modules/new_coin/domain.rs`: old ImmediateOnListing is timestamp-based.
  New project-gated semantics must not change old standalone domain contracts.
