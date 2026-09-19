# N05 Margin Numeric Safety

## Migration Semantics (Reserved 0141)

- `0141_margin_interest_remainder.sql` adds `interest_remainder DECIMAL(26,26)`.
  Existing rows start at exactly zero. No changes to accrued debt, interest
  checkpoints, position principal, wallet balances, or historical journal legs.
- Future accrual uses the existing checkpoint, not the original lifetime of
  the position: `raw = borrowed * snapshot_rate * whole_hours + remainder`;
  quantize the new delta once to authoritative `assets.precision_scale`, keep
  `raw - delta` exactly. Principal has 18 and rate 8 decimals, so 26 places
  suffice without another truncation.
- Checkpoint and remainder advance atomically even when the delta is zero.
  There is no attempt to reconstruct or backcharge historical lost fractions.
- A partial close first accrues completed hours at the old principal. It
  allocates already accrued debt, preserves the existing sub-unit remainder
  on the continuing position, and then reduces principal. Final close and
  liquidation do not charge the remaining sub-unit carry; the terminal row
  retains it for audit. The existing whole-hour policy remains; an incomplete
  hour crossing a partial close subsequently uses the remaining principal.
- Deployment must apply 0141 before the new API/workers and retire all old
  writers together. Mixed old/new interest workers are unsupported.

## Implementation Checklist

- [x] Open input asset/storage precision and generated notional envelope.
- [x] Partial-close asset allocation, exact remainders and canonical PnL.
- [x] Shared interest adapter, forward carry and checked duration.
- [x] Settlement/transfer balance envelope without bucket re-quantization.
- [x] Isolated/cross liquidation canonical PnL and journal consistency.
- [x] Closest Rust unit, real isolated DB, architecture/docs and owned format gates.
- [x] Final file inventory, exact command evidence and limitations.

## Boundary Inventory

- `application/open_position.rs`: positive source collateral, DECIMAL(38,18)
  and authoritative asset precision; leverage DECIMAL(18,8); generated
  notional quantized once, overflow rejected before insertion/debit.
- `infrastructure/positions.rs`: assets joined into locked open product;
  authoritative ticker fill price storage validation.
- `application/lifecycle.rs`, `domain.rs`: partial percentages use asset scale,
  exact subtraction retains historical dust, full close consumes original
  stored values. New PnL quantized once, reused by execution/wallet/journal.
- `infrastructure/interest.rs`, `workers/margin_interest.rs`: exact 26-place
  forward carry, original rate snapshots, no floating-point finance; checked
  timestamp advance is identical to the billed hour count.
- `workers/margin_liquidation.rs`: original unquantized risk trigger preserved;
  execution PnL quantized once. Account execution aggregates the same per-leg
  PnL for records, bad debt and journal legs. Historical wallet available is
  consumed as originally required by cross liquidation, not rounded away.
- `infrastructure/settlement.rs`, `transfers.rs`, `cross_accounts.rs`: checked
  destination balance/risk/aggregate capacity; no bucket rounding.
- `application/product_config.rs`: reuse shared storage-envelope helper,
  preserving narrower rate/leverage columns and business policy.
- `infrastructure/market_data.rs`: cached prices must fit storage before use.
- Cache ticker decimal deserialization is bounded too; dynamic stored leverage
  strings use the shared bounded parser plus their narrower 18,8 envelope.
- Interest checkpoints and liquidation scheduling use
  `time::ensure_timestamp_storage`; an unrepresentable future checkpoint fails
  the transaction instead of being left to MySQL or clamping billed hours.
- No changes to production data, PROGRESS, indexes, task metadata, or request
  DTOs (the main owner supplies bounded request deserializers).

## Final Validation

Main supplied isolated MySQL 9.3 on loopback 13316 (`numeric_margin_test`) and
Redis 16386 database 3. This is not a production MySQL 8.4 migration rehearsal.
No services are started or stopped by this owner.

```sh
env DATABASE_URL=mysql://root@127.0.0.1:13316/numeric_margin_test \
  REDIS_URL=redis://127.0.0.1:16386/3 \
  cargo test --test margin_routes --test margin_liquidation_worker -- --nocapture --test-threads=1

env MARGIN_CONCURRENCY_DATABASE_URL=mysql://root@127.0.0.1:13316/numeric_margin_test \
  MARGIN_CONCURRENCY_REDIS_URL=redis://127.0.0.1:16386/3 \
  cargo test --lib margin -- --nocapture --test-threads=1

cargo test --test backend_architecture --test backend_documentation
```

- Final whole-file DB regression: **43/43 routes and 12/12 worker tests**,
  exit 0 (session `61372`). This includes the original close, partial-close,
  interest, isolated/cross liquidation, wallet-scope, replay and risk cases;
  environment-dependent DB branches were not skipped.
- Final selected library regression: **60/60**, exit 0, including both actual
  optional concurrency DB branches using their dedicated environment variables.
  Earlier 56/59-test runs without these variables are not DB evidence.
- Architecture **11/11** and documentation **1/1**, exit 0 (session `12546`).
  No N05 test sessions remain running.
- Main separately reported full library **500/500** and strict Clippy passed.
  These are shared-owner results, not an additional N05 rerun.
- All owned Rust paths passed `rustfmt --check --edition 2024 --config
  skip_children=true`; scoped `git diff --check` passed. The final local global
  `cargo fmt --all -- --check` still reported concurrent non-owned Spot files
  and `tests/earn_routes.rs`, with no Margin differences. Other owners retain
  responsibility for the global formatting gate.
- Initial sandbox socket denial was rerun with approval. An earlier full
  route run had four fixture failures, all corrected before the final pass:
  use a real admin plus scoped audit failure injection, authenticate the
  invalid-settings HTTP request and separately test application validation
  without DB access, expect 422 for extraction-time decimal rejection, and
  decode actual TIMESTAMP snapshots as `DateTime<Utc>`. No final N05 failures.

### Actual Interest And Migration Evidence

- `margin_numeric_interest_carry_batches_zero_delta_replay_and_overflow`
  starts from existing debt `4.000000000000000001`, principal `0.00000000015`,
  rate `0.00000001`. After one hour the persisted debt is
  `4.000000000000000002` and carry `0.0000000000000000005`; two one-hour batches
  and one two-hour batch both persist debt `4.000000000000000004`, carry zero.
  Replaying the same cutoff adds nothing.
- The asset-8 fixture preserves its existing 18-place debt while the first
  generated increment is zero: carry becomes `0.0000000015`, and the persisted
  checkpoint advances exactly one hour. The overflow fixture reports a
  per-position failure and leaves debt, carry and checkpoint unchanged.
- `margin_numeric_input_envelopes_partial_settlement_dust_and_replay`
  exercises both modes, 37% partial close, replay, final close, existing dust,
  and zero-increment accrual before reducing principal. Original
  `margin_partial_close_*`, `cross_margin_partial_close_*` and complete-close
  regressions also pass in the whole-file run.
- Both new asset-8 liquidation tests compare persisted record PnL, wallet
  changes and balanced platform legs. Cross liquidation uses the same position
  PnL in the account journal; old isolated/cross liquidation tests remain green.
- The actual migrator applied 0141 in the isolated schema; later migrator runs
  also succeeded. Final read-only metadata checks returned MySQL `9.3.0`,
  `_sqlx_migrations.version=141, success=1`, column `decimal(26,26)`, nullable
  `NO`, default `0.00000000000000000000000000`, and enforced check expression
  `interest_remainder >= 0 AND interest_remainder < 1`.
- Metadata was checked with `mysql --protocol=TCP -h 127.0.0.1 -P 13316 -u root
  numeric_margin_test --batch --execute=...`, reading `VERSION()`,
  `_sqlx_migrations`, `information_schema.COLUMNS`, and
  `information_schema.CHECK_CONSTRAINTS`. No live business rows were changed.

## Residual Boundaries

- Production MySQL 8.4 migration/DDL-lock rehearsal remains a release gate;
  MySQL 9.3 results do not establish production-version compatibility.
- Apply 0141 before all new writers, then retire old interest writers together.
  No historical remainder reconstruction, balance rewriting, or backcharge.
- Terminal sub-asset carry is retained only for audit, not charged. Partial
  close preserves carry on the remaining position and keeps the existing
  incomplete-hour billing policy described above.
- Main owns the request DTO boundary. Final read-only inspection confirms both
  `CreateMarginProductRequest` and `UpdateMarginProductRequest` now use Main's
  `deserialize_optional_decimals` for leverage collections. Main also added
  list/null/empty/overflow tests and collection-aware AST coverage; their final
  formatting and rerun remain Main's gate. N05's stored `Vec<String>` parser
  is bounded and covered by the completed N05 regression.
- No production operation, commit, service lifecycle change or shared-file
  modification was performed. Main owns PROGRESS and task completion metadata.

## Owner Path Inventory

This lists N05-owned changes only, not other owners' edits to the same dirty
worktree. Existing changes in these paths were preserved.

```text
src/modules/margin/amounts.rs
src/modules/margin/mod.rs
src/modules/margin/domain.rs
src/modules/margin/application/account_settings.rs
src/modules/margin/application/lifecycle.rs
src/modules/margin/application/open_position.rs
src/modules/margin/application/product_config.rs
src/modules/margin/application/support.rs
src/modules/margin/application/trigger_limit_orders.rs
src/modules/margin/infrastructure.rs
src/modules/margin/infrastructure/cross_accounts.rs
src/modules/margin/infrastructure/interest.rs
src/modules/margin/infrastructure/market_data.rs
src/modules/margin/infrastructure/positions.rs
src/modules/margin/infrastructure/settlement.rs
src/modules/margin/infrastructure/transfers.rs
src/workers/margin_interest.rs
src/workers/margin_liquidation.rs
tests/margin_routes.rs
tests/margin_liquidation_worker.rs
tests/margin/numeric.rs
tests/margin/numeric_workers.rs
tests/unit_src/src_modules_margin_amounts_tests.rs
tests/unit_src/src_modules_margin_interest_tests.rs
tests/unit_src/src_modules_margin_open_position_tests.rs
tests/unit_src/src_workers_margin_interest_tests.rs
tests/unit_src/src_workers_margin_liquidation_tests.rs
migrations/0141_margin_interest_remainder.sql
.trellis/spec/backend/margin-trading-actions.md
.trellis/tasks/09-19-numeric-safety-hardening/research/n05-margin.md
```

Main owns `src/numeric.rs`, `src/time.rs`, and request-deserializer edits to
`margin/presentation.rs`, including the collection adapter, its tests/AST guard
and additional backend presentation documentation. They are dependencies, not
N05-owned modifications, and are excluded from the 30-path inventory.
