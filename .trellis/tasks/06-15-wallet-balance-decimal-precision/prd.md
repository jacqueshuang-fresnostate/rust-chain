# Fix Wallet Balance Decimal Precision

## Problem

Wallet balances can receive calculated amounts with more fractional digits than the target asset allows. A reported BTC balance example is `0.019600192108874474`, which matches a generated conversion amount stored at full `DECIMAL(38,18)` scale instead of BTC-style asset precision.

## Scope

- Use the existing `assets.precision_scale` field as the asset amount precision contract.
- Quantize generated convert quote amounts before they are returned, cached, inserted into `convert_quotes`, copied into `convert_orders`, and applied to `wallet_accounts`.
- Keep the fix local to calculated wallet amounts and avoid changing unrelated order, admin, or frontend behavior.

## Acceptance Criteria

- Generated target amounts in convert quotes are truncated to the target asset `precision_scale`.
- Convert source fee amounts are truncated to the source asset `precision_scale`.
- Confirming a convert quote writes target wallet `available` and wallet ledger snapshots at the target asset precision.
- Source amounts with too many fractional digits for the source asset are rejected instead of silently stored.
- Regression coverage demonstrates a market-priced reverse convert that would previously produce an 18-decimal BTC-like amount now stores an 8-decimal amount.

## Notes

- `wallet_accounts` and `wallet_ledger` remain `DECIMAL(38,18)` for storage compatibility; precision control happens at the calculation boundary.
- Existing assets default `precision_scale` to 8, and admin validation limits it to `0..=18`.
