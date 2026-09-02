# Admin / Mobile financial display precision audit

## Reported symptom and root cause

The reported `1,134.331253942506787192 USDT` value is not an integer overflow.
The backend intentionally keeps realized-return calculations as `BigDecimal` and serializes
18 fixed decimal places. The Mobile presentation then explicitly requests 18 visible fraction
digits, so calculation/storage precision leaks into the UI.

The exact direct path is:

1. `src/modules/wallet/application.rs` calculates UTC realized return and quantizes it to 18
   decimal places for auditability.
2. `src/modules/wallet/presentation.rs::TodayReturnResponse` serializes `amount`,
   `basis_amount`, and `rate` through `serialize_decimal_18`.
3. `mobile/src/core/todayReturn.ts` correctly preserves those values as `DecimalText`.
4. `mobile/src/core/todayReturnPresentation.ts` formats the amount with
   `maximumFractionDigits: 18` and appends `USDT`.
5. Both `HomeView.vue` and `AssetsView.vue` render that shared presentation.

The backend precision must remain intact. The defect is confined to the final presentation
boundary.

## Other Mobile leak points

- `mobile/src/views/HomeView.vue`: return-history summary uses 18 visible digits.
- `mobile/src/views/{Withdraw,NewCoinDetail,QuickRecharge,Loan,Earn}View.vue`: local
  `formatMoney` helpers use 18 visible digits.
- `mobile/src/core/tradeFinancial.ts`: bound `formatValue` defaults to 18 digits.
- `mobile/src/core/secondsFinancial.ts`: bound `formatValue` defaults to 18 digits.
- `mobile/src/core/walletLedger.ts`: correctly consumes authoritative `precision_scale`, but
  uses it directly as visible digits. A stored/business precision of 18 therefore still creates
  an unreadable row.
- `mobile/src/core/transactionRecords.ts`: already caps normal values at eight digits and is not
  the direct source of the 18-digit USDT example.

Input-side `maxScale: 18`, request snapshots, DTO adapters, exact price helpers, and decimal
arithmetic tests are not presentation defects and must not be reduced.

## Admin leak points

- `web/src/shared/numberFormat.ts` documents `0,0.00[0000]` but delegates to an unlimited
  formatter.
- `web/src/shared/decimal.ts::formatDecimalText` pads to a minimum precision and deliberately
  preserves every returned fraction digit. It has no visible maximum or rounding policy.
- `web/src/shared/AmountText.tsx`, generic resource amount cells,
  `DetailDrawer.tsx`, inline amount cells, market latest prices, and agent pages inherit that
  behavior.
- CSV creation in `AdminResourcePage.tsx` uses the raw record and must stay unchanged.

## Recommended presentation contract

1. Keep all financial source values as decimal text. Never parse the coefficient with
   JavaScript `Number`.
2. Separate visible precision from storage/input precision:
   - stablecoins and fiat-like amounts: at most 2 fraction digits;
   - non-stable asset quantities: at most 8 fraction digits;
   - unknown/generic Admin financial values: minimum 2 and at most 6 fraction digits;
   - percent values: at most 2, or 4 where a rate field explicitly needs it;
   - market prices: explicit pair price precision where available, otherwise the existing
     price-specific adaptive formatter.
3. Use deterministic decimal half-up rounding only for rendered text. Do not mutate the source
   value or reuse the rendered value in a request.
4. Normalize rounded negative zero to zero.
5. If a non-zero magnitude is below the smallest visible unit, render a threshold (`<0.01`,
   `>-0.01`, `<0.00000001`, etc.) instead of false zero.
6. Preserve raw values in API models, mutation payloads, exact execution snapshots, and CSV
   exports.

## Affected implementation files

### Mobile core

- `mobile/src/core/decimal.ts`
- `mobile/src/core/todayReturnPresentation.ts`
- `mobile/src/core/walletLedger.ts`
- `mobile/src/core/tradeFinancial.ts`
- `mobile/src/core/secondsFinancial.ts`
- a new shared presentation helper under `mobile/src/core/`

### Mobile views

- `mobile/src/views/HomeView.vue`
- `mobile/src/views/WalletLedgerView.vue`
- `mobile/src/views/WithdrawView.vue`
- `mobile/src/views/NewCoinDetailView.vue`
- `mobile/src/views/QuickRechargeView.vue`
- `mobile/src/views/LoanView.vue`
- `mobile/src/views/EarnView.vue`

### Admin

- `web/src/shared/decimal.ts`
- `web/src/shared/numberFormat.ts`
- `web/src/shared/AmountText.tsx`
- callers/tests that currently assert unlimited visible fractions

## Test matrix

| Case | Expected presentation property |
| --- | --- |
| `1134.331253942506787192`, USDT | `1,134.33` |
| `-1134.335`, USDT | deterministic half-up result, no negative-zero artifact |
| `999.999`, USDT | carry into grouped integer (`1,000.00` when fixed two digits) |
| `0.000000001`, BTC | threshold instead of `0` |
| `1.234567895`, BTC | eight-place half-up rounding |
| integer beyond `Number.MAX_SAFE_INTEGER` | exact grouped integer, no precision loss |
| Admin scientific `1e-18` | accepted by existing adapter, rendered as threshold |
| malformed decimal | existing unavailable/error behavior |
| CSV/API/request payload | original unformatted decimal remains byte-for-byte unchanged |

## Verification

- Mobile focused decimal/today-return/wallet/trade/seconds tests, then `npm --prefix mobile run release:gate`.
- Admin shared formatter/resource tests, then typecheck, lint, full tests, production-policy,
  coverage, build, and budget gates.
- `git diff --check`.

## Implemented outcome

- Mobile now owns a shared asset-aware final-render formatter. Stable/fiat-like amounts cap at
  two fraction digits, other assets at eight, and lower validated asset precision can tighten
  the cap. The exact `DecimalText` remains unchanged.
- Today return, return-history summary, wallet ledger, withdrawal, loan, earn, new-coin,
  quick-recharge, trade, and seconds-contract display paths no longer request 18 visible digits.
- Admin generic values cap at six digits; `AmountText` and generic resource rows choose an
  asset-aware cap without duplicating the asset label. Price/rate/ratio/leverage fields are not
  accidentally classified as wallet amounts, and CSV still serializes the raw record.
- Both implementations perform decimal coefficient half-up rounding and threshold rendering
  without IEEE-754 conversion.
