# Margin Ledger Data Flow

## Current flow

```text
spot wallet mutations    -> wallet_ledger
margin wallet mutations  -> margin_wallet_ledger
GET /wallet/ledger       -> wallet_ledger only
mobile /assets/ledger    -> GET /wallet/ledger
```

- `src/modules/wallet/infrastructure/accounts_ledger.rs` builds both the list
  and count query from `wallet_ledger`.
- `src/modules/margin/infrastructure/ledger.rs` deliberately writes
  margin-funded actions to `margin_wallet_ledger`.
- Spot-to-margin and margin-to-spot transfers write one row to each table.
  These are two account-side movements, not duplicate records.
- Margin-scope open, close, cancel, and liquidation rows can exist only in
  `margin_wallet_ledger`; the current endpoint therefore hides them.

## Chosen read contract

- Keep both physical ledgers and all mutation paths unchanged.
- Build one read-only `UNION ALL` source that assigns an authoritative
  `account_type` literal to each branch:
  - `wallet_ledger` -> `spot`
  - `margin_wallet_ledger` -> `margin`
- Add optional `account_type=all|spot|margin`; omission is `all`.
- Validate the account filter before acquiring the database pool, matching the
  existing category validation boundary.
- Apply asset/change/ref/time/category filters consistently to the combined
  source, and use the same predicate semantics for list and count.
- Order the combined result globally by `created_at DESC`, then
  `account_type`, then `id DESC` before applying offset/limit. Ordering only
  each branch would make cross-table pagination unstable.
- Preserve category as a separate business dimension. For example,
  `category=margin&account_type=spot` is valid and returns the spot side of
  margin-related wallet movements.

## Identity and mobile mapping

The two tables have independent auto-increment sequences, so numeric `id` is
not globally unique. Backend compatibility can preserve the numeric field, but
mobile list merge, deduplication, and Vue keys must use
`accountType + ":" + id`.

The mobile adapter must reject an unknown or absent `account_type` rather than
infer it from `change_type`; a `margin_*` business event can belong to either
the spot or margin account.

## Test touchpoints

- Backend unit tests: account filter parser, shared SQL predicates, union
  source, account response mapping, and deterministic order.
- Backend route tests: malformed account filter returns 400 before database
  access.
- Backend integration test: seed one row in each table (including equal
  numeric IDs where practical), assert default union, spot/margin filters,
  combined category filtering, total, decimal serialization, and cleanup.
- Mobile tests: DTO mapping, API query, account/category lifecycle, stale
  response guard, composite identity merge, i18n symmetry, and view source
  contract.

## Non-goals

- No migration or historical backfill.
- No changes to margin settlement, transfer, or liquidation write paths.
- No account-side netting of paired transfer rows.
