# Financial Journal Coverage

## 1. Scope

Actual money mutations in Seconds, Prediction, Convert, Margin and Spot extend
the existing Wallet/Earn/Loan/New-coin platform journal. A journal is accounting
evidence, not a custody balance, an authorization to mint funds or proof of
solvency. Historical coverage remains partial; no synthetic opening backfill.

## 2. Signatures

- `wallet::infrastructure::insert_wallet_platform_journal_legs_in_tx(...)`
  preserves numeric reference metadata for existing callers.
- `insert_platform_journal_with_reference_in_tx(...)` also supports quote IDs
  and composite cross-margin references without changing their JSON type.
- Existing `platform_financial_journal` uniqueness is
  `(transaction_key, account_code, asset_id)`.

## 3. Contracts

- Every transaction's legs sum to exactly zero **per asset**. Never offset BTC
  against USDT or use a floating-point conversion to hide a difference.
- Inserts share the original order, wallet, ledger and audit transaction.
  Duplicate or unbalanced legs fail that transaction; no `INSERT IGNORE`.
- Existing business replay short-circuits before journal insertion. Journal
  uniqueness is a final guard, not permission to apply a wallet mutation twice.
- Debit is positive and credit negative. Zero legs are omitted.
- Seconds open moves wallet liability to pending-order liability, not revenue.
  Settlement closes pending liability and records actual payout plus net P&L,
  including zero-payout losses and evidence-based manual recovery.
- Prediction principal remains in the user's frozen wallet at opening.
  Only the fee is revenue then; final settlement debits frozen liability,
  credits actual payout and records net P&L. Invalid refunds only reverse the
  actual returned fee; unfreezing principal is internal.
- Convert source debit is split between net inventory and the fee already
  included in that debit. Target inventory and wallet liability are recorded
  separately. A source fee is never charged twice or booked in the target asset.
- Margin opening/cancellation moves collateral liability. Each close execution
  books only its allocated collateral, actual signed payout, allocated P&L and
  interest. Isolated loss truncation is a bad-debt expense; cross liquidation
  aggregates current remaining positions and consumes shared available once.
  Positive residual equity follows the existing liquidation contract, and
  account bad debt must not also be added once per liquidated position.
- Manual user-to-user Spot fills transfer liabilities; only the existing
  prepaid platform liquidity account counts as platform inventory. Upstream
  market providers are not financial counterparties. Current zero-fee fills
  stay zero-fee; nonzero fees require a real charged-asset settlement contract.

## 4. Error Matrix

| Condition | Result |
| --- | --- |
| Nonzero per-asset leg sum | Internal error, full rollback |
| Duplicate leg | Database conflict/error, full rollback |
| Exact business replay | Original result; no new legs |
| Historical opening missing | Keep missing; report partial coverage |
| Wallet or final audit write fails | Order and journal roll back together |
| No custody evidence | Never infer externally available funds |

## 5. Cases

- Good: a partial margin close and later terminal close use different keys and
  together debit precisely the original collateral.
- Base: a legacy Seconds order settles today with only settlement legs; its
  historical open is reported as missing rather than invented.
- Bad: compare whole-platform `SUM(amount)=0`, conceal per-transaction
  imbalances or fund inventory using journal net movement.

## 6. Tests

Pure constructors cover profit/loss/zero and opposite reversals. Real MySQL
tests check per-asset sums, exact fee and wallet amounts, partial slices,
terminal replay, worker replay and duplicate-leg rollback. Convert failure
must leave the quote unconsumed, no order and no newly created target account.

## 7. Wrong Vs Correct

Wrong: update wallet, commit, then append best-effort journal entries.

Correct: lock the business object, validate replay, mutate wallet and order,
insert balanced legs and required audit, then commit once.
