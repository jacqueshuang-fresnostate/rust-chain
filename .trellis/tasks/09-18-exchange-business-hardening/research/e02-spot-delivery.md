# E02 Spot Journal Integration

## Scope And Tracking

- [x] Inspect actual manual/automatic wallet settlement and the shared journal helper.
- [x] Add per-asset journal writes inside all three existing fill transactions.
- [x] Verify manual replay/audit rollback and real automatic inventory settlement.
- [x] Record final checks and hand back E07/journal implementation; E03 now owns ongoing spot edits.

The existing fill fee is exactly zero. This slice does not introduce a fee,
additional debit, fee asset inference or an upstream exchange counterparty.
Actual user-to-user transfers have opposite source/target liability legs.
Only the existing internal prepaid liquidity account is classified as platform
inventory; its actual wallet change has the opposite inventory counterleg.
Zero legs are omitted. A future nonzero trade fee fails closed until an explicit
charged-asset/net-settlement contract is implemented.

## Changed Paths

- `src/modules/spot/infrastructure/fill_journal.rs`
- `src/modules/spot/infrastructure.rs`
- `src/modules/spot/application/{settlement,triggering}.rs`
- `tests/unit_src/src_modules_spot_fill_journal_tests.rs`
- `tests/spot/manual_fill.rs`
- `tests/spot/fill_journal.rs`
- `tests/spot_routes.rs` (inventory fixture ID readback and journal cleanup before fixture assets)

Uses `insert_platform_journal_with_reference_in_tx` with context `spot`,
key `spot:{trade_id}:fill`, reference type `spot_trade` and the real trade ID.
Each asset is independently balanced. Existing trade replay short-circuits
before journal writes; any audit/journal failure rolls back financial effects.

## Checks

- Pure journal unit test passed, including exact 18-place quantities, user-to-user,
  each inventory direction and zero-leg omission.
- Isolated MySQL 9.3 + Redis, escalated execution, schema `e03_spot_trigger_test`:
  automatic buy/sell inventory and journal failure injection passed; manual audit
  rollback and ten concurrent replays passed; market buy/sell and triggered limit
  buy/sell regression passed. The final full spot suite passed **62/62**, zero
  failures/skips, including all E02/E03/E07 cases. Earlier old Admin-list
  fixture failures (duplicate-insert ID and hardcoded admin) were corrected;
  concurrent non-spot compilation blockers cleared before the successful run.
  See `e03-delivery.md` for final integration needs and the exact command.
- The old `fund_system_spot_liquidity` test helper assumed duplicate INSERT
  returned a valid ID. Real DB foreign-key errors exposed this; it now reads the
  stable internal account ID by the existing unique email. Production funding
  and liquidity rules were not altered.
- Shared `cargo check --all-targets` passed after the E03 fields were connected.
- `git diff --check` passed. No migration is needed for E02.
