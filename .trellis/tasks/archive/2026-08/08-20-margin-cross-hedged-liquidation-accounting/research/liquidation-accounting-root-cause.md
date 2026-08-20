# Research: Cross-margin hedged liquidation accounting root cause

- Query: Trace why cross-margin hedged long+short liquidation credits the margin wallet instead of consuming collateral, from worker/domain calculation through wallet and ledger writes; cover locking, PnL, margin, principal, interest, fees/insurance, duplicate/sign risks, affected files, and tests.
- Scope: internal
- Date: 2026-08-20

## Findings

### Executive conclusion

The observed positive wallet entry is deterministic in the current model. It is not a second per-position wallet credit: cross liquidation performs one account-level wallet mutation. The root cause is that liquidation reuses a close-style settlement amount:

```text
P = sum(position margin + signed PnL - accrued interest)
E = wallet available before liquidation + P
wallet available after liquidation = max(0, wallet available before liquidation + P)
```

`P` includes every position's original margin. When long and short PnL offset, `P` can remain positive at the liquidation boundary. `apply_cross_margin_account_settlement` then adds that positive value to `margin_wallet_accounts.available` and writes a positive `margin_cross_account_liquidate` ledger entry (`src/modules/margin/domain.rs:265-269`, `src/workers/margin_liquidation.rs:762-790`, `src/modules/margin/infrastructure/settlement.rs:306-346`).

There is no liquidation-fee or insurance-fund leg to receive residual equity. Therefore all positive residual portfolio equity is returned to the user wallet. This matches the repository's existing signed-portfolio settlement contract (`.trellis/tasks/07-13-trading-agent-hierarchy/research/p0-financial-safety-design.md:5-13`) but conflicts with the requested policy that forced liquidation consume residual collateral. The defect is thus primarily a settlement-policy/model gap, not an accidental duplicate SQL update.

### Minimal numeric reproduction

For two same-pair positions after their opening debits:

```text
wallet available W = 0
long:  margin 20, notional 100, entry 100, mark 80, interest 15
short: margin 20, notional 100, entry 100, mark 80, interest 15
maintenance rate = 5% per position

long PnL  = 100 * (80 - 100) / 100 = -20
short PnL = 100 * (100 - 80) / 100 = +20
portfolio P = (20 - 20 - 15) + (20 + 20 - 15) = 10
maintenance MM = 5 + 5 = 10
account equity E = W + P = 10, so E <= MM liquidates at equality
current wallet after = max(0, 0 + 10) = 10
```

The worker therefore writes a `+10` margin-wallet ledger entry. The allocation helper assigns the audit-only payout to the positive-equity short leg, while the long leg gets zero (`src/modules/margin/domain.rs:286-331`). Relative to the user's pre-open 40 collateral, this is not a mint—the net loss is 30—but it is a residual-collateral refund. With a consume-on-liquidation policy, that 10 must instead be explicitly booked to an insurance/platform account (or as a configured liquidation fee), leaving no unexplained positive user-wallet delta.

### Exact write flow

#### 1. Opening establishes the accounting basis

1. `open_margin_position` calculates `notional = margin * leverage` and `borrowed = max(notional - margin, 0)` (`src/modules/margin/application/open_position.rs:133-135`, `src/modules/margin/application/open_position.rs:366-370`).
2. It inserts the position first, then debits the cross margin wallet (`src/modules/margin/application/open_position.rs:154-191`).
3. Cross opening always locks `margin_wallet_accounts`, subtracts only `margin_amount` from `available`, and writes one negative `margin_position_open` ledger row (`src/modules/margin/infrastructure/settlement.rs:32-71`).
4. `wallet_scope` is persisted as `margin`, and `(user_id, margin_asset)` gets one cross-account row (`src/modules/margin/infrastructure/positions.rs:370-406`).

The position margin is therefore no longer in wallet `available`; it exists only as `margin_positions.margin_amount`. Returning `margin_amount + PnL - interest` when the position disappears is close-style realization.

#### 2. Candidate and mark-price phase is outside the settlement transaction

1. The worker runs by default every five seconds when MySQL and Redis are configured (`src/main.rs:196-210`, `src/config.rs:331-346`).
2. Cross candidates are grouped by `(user_id, margin_asset)` and include only filled, opened cross positions (`src/workers/margin_liquidation.rs:479-503`).
3. All account positions are listed before the transaction; each position then reads Redis separately (`src/workers/margin_liquidation.rs:333-363`, `src/workers/margin_liquidation.rs:506-530`). Missing any mark skips the whole account.

For same-symbol long and short positions, Redis is still read once per position. A ticker update between reads can give the two hedge legs different marks. The implementation therefore does not provide the exact same market snapshot claimed by its comments; it only requires each independently read mark to be positive and no older than 60 seconds (`src/workers/margin_liquidation.rs:533-555`). This is a secondary hedged-account valuation risk.

#### 3. Cross liquidation transaction and lock order

`liquidate_cross_account` owns one MySQL transaction (`src/workers/margin_liquidation.rs:681-688`):

1. It locks every currently filled/opened cross position for the account with `FOR UPDATE`, ordered by position id (`src/workers/margin_liquidation.rs:689-705`).
2. It then locks the margin-wallet row with `FOR UPDATE` and reads `available`; a missing row is interpreted as zero (`src/workers/margin_liquidation.rs:711-722`).
3. It validates every position has `wallet_scope = margin`, an entry price, and a supplied mark (`src/workers/margin_liquidation.rs:726-739`).
4. It computes signed PnL per leg—long uses `mark-entry`, short uses `entry-mark`—and gross maintenance per leg as `notional * maintenance_rate` (`src/workers/margin_liquidation.rs:245-268`, `src/workers/margin_liquidation.rs:740-760`).
5. Domain aggregation sums all margins, PnL, interest, and maintenance. It does not apply long/short hedge offsets to maintenance; maintenance is gross across both legs (`src/modules/margin/domain.rs:242-283`).
6. The worker writes the pre-settlement cross-risk snapshot. If `E > MM`, it commits only that snapshot; if `E <= MM`, it continues (`src/workers/margin_liquidation.rs:762-767`, `src/workers/margin_liquidation.rs:871-901`).
7. It calculates audit payouts from positive position equities, capped by positive `portfolio_equity` (`src/workers/margin_liquidation.rs:769-778`).
8. It calls the concrete infrastructure adapter directly—there is no margin repository port in this path (`src/workers/margin_liquidation.rs:21-23`, `src/modules/margin/infrastructure.rs:37-40`).
9. `apply_cross_margin_account_settlement` locks/ensures the same wallet again, computes `raw_after = available + portfolio_equity`, clamps a negative result to zero, updates `available`, and inserts one ledger row for the actual applied delta (`src/modules/margin/infrastructure/settlement.rs:306-351`).
10. For each locked position, the worker inserts one `margin_liquidation_records` row and updates the position to `liquidated` with exit price and realized PnL (`src/workers/margin_liquidation.rs:794-848`). These per-position `payout_amount` values do not call a wallet function.
11. It updates `margin_cross_accounts` to `liquidated`, stores post-settlement wallet balance in `last_equity`, stores only negative overshoot as `last_bad_debt`, then commits (`src/workers/margin_liquidation.rs:849-862`).
12. User events are published only after commit (`src/workers/margin_liquidation.rs:363-372`, `src/workers/margin_liquidation.rs:904-930`).

The position-first, wallet-second order matches active close and prevents close/liquidation from settling the same locked position twice. A competing worker waits and subsequently finds no opened positions. However, `margin_wallet_ledger` has no unique business-key constraint (`migrations/0079_margin_user_actions.sql:19-36`); exactly-once currently relies on row locks and terminal position states rather than a database-enforced liquidation batch identity.

### PnL, returned margin, and double-credit/sign assessment

- Signed PnL is correct for linear quote-margin exposure: equal-notional long and short at the same entry and same mark cancel exactly (`src/workers/margin_liquidation.rs:251-268`).
- Cross maintenance is not hedged; it sums both legs' gross notional maintenance (`src/modules/margin/domain.rs:259-264`). This can liquidate a delta-neutral account as interest accumulates.
- The current cross path does not call `credit_margin_position_amount` per position. It calls the account settlement once, so the audit payout rows are not a second wallet credit (`src/workers/margin_liquidation.rs:785-848`).
- The worker correctly passes `portfolio_equity`, not account `equity`, to settlement (`src/workers/margin_liquidation.rs:785-790`). Passing account equity would double-count existing wallet available; the current code avoids that specific sign/double-add bug.
- The policy error is that positive `portfolio_equity` is treated as user refund during forced liquidation. No amount is diverted to liquidation fees or insurance.
- `payout_amount` is misleading for cross records/events: it is an audit allocation, not an independently paid amount (`src/workers/margin_liquidation.rs:1002-1007`). Any downstream consumer treating it as a second payment would double-count, although no such wallet consumer was found in the current backend.

### Borrowed principal

`borrowed_amount` is persisted as `notional - margin` and described by the schema as borrowed principal (`migrations/0031_margin_borrow_interest.sql:5-11`), but no principal is transferred at open and no principal is repaid at close or liquidation. It is only:

- returned in position/read DTOs;
- used as the interest-worker base (`src/workers/margin_interest.rs:159-183`, `src/workers/margin_interest.rs:219-223`);
- stored on the position (`src/modules/margin/infrastructure/positions.rs:326-367`).

The cross-liquidation lock/query does not even load `borrowed_amount` (`src/workers/margin_liquidation.rs:128-143`, `src/workers/margin_liquidation.rs:689-700`), and liquidation records have no principal field (`migrations/0027_margin_liquidation_records.sql:1-34`). Thus the backend currently implements synthetic leveraged PnL with a notional-based interest charge, not an auditable borrowed-asset principal/repayment ledger. If the product is intended to be true margin borrowing, omitted principal settlement is a separate structural accounting defect.

### Interest cutoff and under-accrual

Liquidation deducts only the `interest_amount` already stored on each locked position. It does not accrue interest through `liquidated_at` inside the liquidation transaction (`src/workers/margin_liquidation.rs:689-700`, `src/workers/margin_liquidation.rs:749-753`). The independent interest worker:

- locks one position at a time;
- charges only complete hours;
- writes the checkpoint to `now`, not `old_checkpoint + charged_hours` (`src/workers/margin_interest.rs:186-263`, `src/workers/margin_interest.rs:291-311`).

Consequences:

1. If liquidation wins before an interest cycle, due full-hour interest is never posted because the now-liquidated position leaves the interest candidate set.
2. Whenever a successful accrual interval contains a fractional hour, advancing the checkpoint to `now` discards that fraction. Existing tests encode 3.5 hours as 3 charged hours, followed by 2.25 hours as 2 charged hours (`tests/margin_liquidation_worker.rs:295-394`).

Both paths understate interest, increase residual `portfolio_equity`, and can enlarge the positive wallet credit.

### Fee, insurance, and counter-account handling

No margin-product liquidation fee, liquidation-record fee, insurance-fund table, insurance wallet, or platform-side liquidation ledger was found. The only loss sink is `margin_cross_accounts.last_bad_debt`, populated when `available + portfolio_equity < 0` (`migrations/0087_p0_financial_safety.sql:1-3`, `src/modules/margin/infrastructure/settlement.rs:313-321`).

The agent commission inserted at position fill is unrelated to liquidation and does not consume liquidation collateral (`src/modules/margin/application/open_position.rs:192-208`). User PnL and residual equity are therefore single-sided user-wallet adjustments without an explicit platform/counterparty balancing entry.

### Product configuration exposure

Maintenance validation only requires a non-negative value; neither application validation nor the original schema limits it below one or below the reciprocal of supported leverage (`src/modules/margin/application/product_config.rs:301-340`, `src/modules/margin/application/product_config.rs:495-508`, `migrations/0022_margin_trading.sql:16-18`). A configuration with `maintenance_rate >= 1 / leverage` can make a freshly opened position liquidatable before any loss. This is not required to reproduce the interest-driven hedged case, but it can amplify or immediately trigger the same positive-refund behavior.

### Required accounting invariant

Capture one locked account snapshot and define:

```text
W = locked pre-liquidation margin-wallet available
M = sum locked position margin
U = sum signed PnL using one mark per symbol/version
I = interest accrued through one liquidation cutoff timestamp
P = M + U - I
E = W + P
MM = sum gross maintenance (or an explicitly approved hedge-adjusted formula)
R = residual returned to the user
C = residual consumed/credited to fee or insurance account
D = platform bad debt

trigger iff E <= MM
conservation: E = R + C - D
user wallet after all account positions close = R
```

For the requested consume-collateral policy, the minimal explicit rule is:

```text
R = 0
C = max(E, 0)
D = max(-E, 0)
```

If product policy instead charges only a liquidation fee, define `C = min(max(E,0), fee_formula)` and `R = max(E-C,0)`. The implementation must not silently choose between these policies. Every liquidation batch must produce exactly one user-wallet delta, one matching insurance/fee entry when `C > 0`, one bad-debt value when `D > 0`, and position/audit totals linked by the same immutable batch id.

Additional invariants:

- Under consume-collateral semantics, a triggered account must never write a positive `margin_cross_account_liquidate` user-wallet amount.
- All same-symbol hedge legs use the identical mark value and version; cross-symbol snapshot skew is bounded and recorded.
- Interest is accrued through the liquidation cutoff under the same position locks; no elapsed full-hour debt survives a terminal transition.
- If `borrowed_amount` denotes a real liability, principal repayment and platform loan-account movement must balance in the same transaction. Otherwise rename/document it as synthetic exposure metadata.
- `SUM(user ledger delta) + SUM(insurance/platform delta) - bad debt` must equal the signed account settlement amount.
- All selected positions transition exactly once; verify each update's affected-row count and enforce a unique liquidation-batch ledger identity.
- Cross record/event `payout_amount` must equal actual user payout allocation, or be renamed to `allocated_portfolio_equity` so it cannot be mistaken for a second payment.

### Minimal regression tests

1. **Direct bug reproduction (required):** same user/asset, equal long+short (`M=20` each, `N=100` each, entry 100, mark 80, interest 15 each), wallet `W=0`, rate 5%. Assert equality liquidation closes both positions in one transaction; under consume policy wallet remains zero, no positive user liquidation ledger exists, and insurance/fee receives exactly 10.
2. **Existing negative-portfolio guard:** retain the current hedged case with `W=60`, `P=-50`; assert one wallet debit of 50, wallet 10, two terminal positions, one account ledger, zero bad debt, and replay creates nothing.
3. **Gap/bad debt:** choose `W + P = -7`; assert wallet zero, bad debt 7, no positive payout, one batch.
4. **Interest cutoff:** make one or more full hours due but unposted, race interest and liquidation with barriers, and assert liquidation charges through its cutoff exactly once with no discarded fractional checkpoint.
5. **Same-symbol mark atomicity:** update Redis between attempted reads for hedge legs and assert both legs use one captured mark/version.
6. **Concurrent idempotency:** run two liquidation workers plus an active close attempt against the same account; assert one batch, one wallet/insurance ledger set, all-or-none position terminal states, and no duplicate events.
7. **Rollback injection:** fail after wallet/ledger write but before the final position/account update; assert wallet, ledgers, records, positions, and cross account all roll back.
8. **Configuration boundary:** reject maintenance rates that make every supported leverage initially liquidatable, unless an explicit product rule permits it.

## Files Found

- `src/main.rs` — starts the liquidation and interest workers.
- `src/workers/margin_liquidation.rs` — candidate scan, mark reads, account locks, risk calculation, settlement orchestration, records, position/account updates, and events.
- `src/modules/margin/domain.rs` — cross equity/portfolio formulas and audit payout allocation.
- `src/modules/margin/application/open_position.rs` — notional/borrowed calculation, position creation, collateral debit, and cross-account creation.
- `src/modules/margin/application/lifecycle.rs` — voluntary cross close; useful comparison because it also applies signed position equity.
- `src/modules/margin/application/product_config.rs` — maintenance-rate validation gap.
- `src/modules/margin/infrastructure/settlement.rs` — opening debit, isolated credit, signed cross close, and account liquidation wallet mutation.
- `src/modules/margin/infrastructure/ledger.rs` — wallet row locks and concrete wallet-ledger inserts.
- `src/modules/margin/infrastructure/positions.rs` — position persistence/locks and cross-account upsert.
- `src/modules/margin/infrastructure.rs` — concrete adapter façade used directly by the worker; no repository abstraction on the liquidation write path.
- `src/workers/margin_interest.rs` — borrowed-principal-based interest accrual and cross-account interest snapshot.
- `src/modules/margin/infrastructure/position_queries.rs` — user-visible cross snapshot reads.
- `src/modules/admin/infrastructure/margin.rs` and `src/modules/admin/presentation/dashboard_audit.rs` — liquidation audit reads; expose payout but no principal/fee/insurance fields.
- `src/modules/wallet/infrastructure/returns.rs` — return history uses `realized_pnl - interest` and has no liquidation-fee/insurance adjustment.
- `migrations/0022_margin_trading.sql` — product/position basis and permissive maintenance constraint.
- `migrations/0027_margin_liquidation_records.sql` — per-position liquidation records; no batch, principal, fee, or insurance fields.
- `migrations/0031_margin_borrow_interest.sql` — borrowed principal and accrued-interest columns.
- `migrations/0079_margin_user_actions.sql` — margin wallet and single-sided ledger schema.
- `migrations/0086_cross_margin_accounts.sql` and `migrations/0087_p0_financial_safety.sql` — shared account snapshot and bad-debt field.

## Existing Tests

- `tests/margin_liquidation_worker.rs:565-695` already uses same-pair long+short with offsetting PnL, but its portfolio equity is negative (`P=-50`), so it proves one debit/no per-position mint and does not cover the positive-residual credit.
- `tests/unit_src/src_modules_margin_domain_tests.rs:133-195` covers aggregate arithmetic, payout cap, and equality liquidation, but not wallet/insurance policy.
- `tests/margin_routes.rs:2080-2250` covers signed voluntary cross close and insufficient shared wallet.
- `tests/margin_liquidation_worker.rs:429-562` covers isolated liquidation payout and replay idempotency.
- `tests/margin_liquidation_worker.rs:699-763` covers recorded wallet scope for isolated liquidation.
- `tests/margin_liquidation_worker.rs:767-917` covers safe-position scheduling, scan rotation, and pending-limit exclusion.
- No existing test covers positive `portfolio_equity` at cross-liquidation time, liquidation fee/insurance transfer, borrowed-principal repayment, exact interest cutoff, same-symbol mark version consistency, or concurrent cross liquidation workers.
- MySQL/Redis integration helpers return early when `DATABASE_URL` or `REDIS_URL` is absent (`tests/margin_liquidation_worker.rs:29-36`, `tests/margin_liquidation_worker.rs:99-117`), so a green default test run may not execute these database assertions.

## External References

- No internet sources were needed; this is a repository-local accounting trace.
- Relevant locked dependency versions: SQLx 0.8.6, BigDecimal 0.4.10, Chrono 0.4.44, Redis client 0.27.6 (`Cargo.lock`).

## Related Specs

- `.trellis/spec/backend/margin-trading-actions.md:35-52` — current wallet-scope, cross-equity, gross-maintenance, account-liquidation, and interest contracts.
- `.trellis/spec/backend/margin-trading-actions.md:86-100` — current required tests; it lacks positive-residual/insurance and cutoff-concurrency cases.
- `.trellis/tasks/07-13-trading-agent-hierarchy/research/p0-financial-safety-design.md:5-13` — explicitly defines the current signed-portfolio refund behavior.
- `.trellis/tasks/07-13-trading-agent-hierarchy/prd.md:61-74` — original account-level settlement and no-per-position-clamping requirements.
- `.trellis/spec/backend/wallet-amount-precision.md` — 18-decimal wallet amount/snapshot precision contract.

## Caveats / Not Found

- The current code and existing spec agree that positive signed portfolio equity returns to the margin wallet. Calling that a defect depends on adopting the requested consume-collateral/insurance policy; this policy must be made explicit before implementation.
- No incident database rows, ledger dump, worker logs, or exact product configuration were supplied, so the report proves the code path and a minimal reproduction rather than matching a specific production account id.
- No insurance fund, liquidation fee, principal-liability ledger, or account-level liquidation batch entity was found.
- This was static research only. No production code was edited and no long-running or environment-dependent tests were executed.
