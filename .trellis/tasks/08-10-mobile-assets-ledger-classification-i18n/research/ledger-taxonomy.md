# Wallet Ledger Taxonomy Audit

## Existing Problem

`WalletLedgerView.vue` maps each visible category to one exact `change_type`:

- deposit -> `deposit`
- trade -> `spot_trade_settlement`
- contract -> `margin_position_open`

Because `/wallet/ledger` paginates before the mobile client sees the rows, client-only grouping cannot produce a complete category page. Category selection must be applied in the database query and its COUNT query.

## Observed Change-Type Families

- Funding: `deposit`, `deposit_*`, `withdrawal_*`, `admin_recharge`, `quick_recharge`
- Spot: `spot_*`
- Margin: `margin_*`
- Seconds contract: `seconds_contract_*`
- Convert: `convert_*`
- Earn: `earn_*`
- New coin: `new_coin_*`
- Loan: `loan_*`
- Prediction: `prediction_*`
- Other/reward: `agent_commission_payout` and future unknown values

Known concrete values include deposit confirmation/reorg reversal, withdrawal reserve/release/confirm, spot freeze/unfreeze/price-improvement/settlement, margin open/close/liquidation/cross-account variants, seconds open/win settlement, convert settlement, earn subscribe/redeem, new-coin payment/lock/unlock, loan collateral/disbursement/repayment, prediction stake/fee/settlement/refund/payout, quick recharge, admin recharge, and agent commission payout.

## Selected Boundary

Expose ten server categories: `funding`, `spot`, `margin`, `seconds`, `convert`, `earn`, `new_coin`, `loan`, `prediction`, `other`. Omission means all rows. Unknown change types always map to `other`.

This preserves the product boundary already required by the mobile UI: spot, margin contracts, and seconds contracts remain separate surfaces.

## Data Flow

`wallet_ledger.change_type` -> backend category classifier -> shared SQL category predicate -> paginated API entry with `category` -> strict mobile adapter -> localized category/type presentation -> local-date groups.

## Edge Cases

- The same category predicate must be used by list and COUNT queries.
- Existing exact `change_type` may be combined with category and remains backward compatible.
- Unknown types remain visible as secondary source text while their primary label is localized.
- Duplicate dates must group by local calendar day, not UTC string slicing.
- Category changes must reset offset and prevent stale in-flight results from replacing the new category.
