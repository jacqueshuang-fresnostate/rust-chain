# P0 Financial Safety Design

## Cross Margin

- Cross collateral is isolated to `margin_wallet_accounts.available`.
- Opening a cross position never falls back to `wallet_accounts`.
- Account equity is wallet available plus signed portfolio equity:
  `sum(margin_amount + unrealized_pnl - interest_amount)`.
- Unified liquidation applies the signed portfolio equity once to the shared
  margin wallet. It must not clamp each losing position independently.
- Position liquidation records may allocate audit-only payout amounts, but
  their sum must equal the positive portfolio settlement amount and wallet
  mutation happens once.

## Withdrawals

- Creation locks one wallet row and moves `amount + fee` from `available` to
  `frozen` in the same transaction as request insertion and ledger creation.
- `(user_id, idempotency_key)` is unique and replays return the original
  request when all immutable fields match.
- Rejection and terminal broadcast failure move the reserved amount from
  `frozen` back to `available`.
- Confirmation removes the reserved amount from `frozen`; no second wallet
  debit occurs at confirmation.
- State transitions use `UPDATE ... WHERE status IN (...)` and inspect affected
  rows to make concurrent retries idempotent.
- The chain event poller consumes withdrawal receipts by stable gateway request
  id. A terminal failure before a transaction hash releases the reservation;
  an ambiguous failure after broadcast keeps funds frozen in `manual_review`.

## Deposits

- `(network, tx_hash, event_index)` is the external event identity.
- Observation upserts confirmation metadata without touching a wallet.
- Crediting locks the event and wallet in one transaction, writes one ledger
  row, and marks `credited_at`.
- Reorg reversal writes a compensating ledger entry once. If available balance
  cannot absorb it, the event becomes `manual_review`.

## Spot Liquidity

- The internal liquidity user is an operational treasury account, not a mint.
- Counter-orders reserve existing inventory through the normal wallet
  reservation path.
- Insufficient treasury inventory aborts the complete user order transaction.
