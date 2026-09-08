# Payout-rate discrepancy — 2026-09-08

## User clarification

Affected client: Mobile. User says 140% includes principal and actual profit rate
should be40%. Do not mask a payout-semantic mismatch with display-only subtraction.

## Read-only evidence

- Mobile configured production origin: https://hipoex.cllbmz.kdns.fr
- One unauthenticated GET /api/v1/seconds-contracts/products?limit=100 succeeded.
  BTC-USDT (product1) and ETH-USDT (product2) both return cycle rates:
  60seconds1.40000000,120seconds1.50000000,180seconds1.60000000,
  300seconds1.80000000. Only public product metadata read; no login/order/funds.
- Backend presentation.rs documents payout_rate excluding principal;
  application.rs copies cycle rate unchanged into order snapshot;
  service.rs seconds_contract_payout_amount computes stake + stake*payout_rate.
- Mobile secondsFinancial.ts formatPayoutRate computes rate*100. Profit calculations
  use stake*rate. PC SecondOptions.vue also uses cycleRate*100. No added principal
  in their percentage formatting. Therefore1.4 is shown140% as supplied, while
  existing backend code would credit240% on a win, not user-intended140% gross.
- This evidence identifies configuration/contract inconsistency, not an arithmetic
  +100 mobile bug. Never globally subtract1 from already-net API fields;0.4 must
  remain40%. Preserve financial truthfulness and original order snapshots.

## Validation / status

Read-only PC adapter test passed1/1 (backend seconds products and orders mapping).
User confirmed the product-rate correction: normalize the affected product and
cycle configuration to net values (1.4 -> 0.4, 1.5 -> 0.5, 1.6 -> 0.6, 1.8 ->
0.8), preserve existing order snapshots, and keep the settlement formula. The
implementation is local-only: add an immutable guarded migration and clarify
mobile/Admin wording; do not mutate the online instance.

## Local implementation and verification

- `migrations/0125_seconds_contract_net_payout_rates.sql` updates only the
  confirmed BTC-USDT/ETH-USDT products whose master row and complete four-cycle
  schedule match the observed gross signature. It maps `1.4/1.5/1.6/1.8` to
  `0.4/0.5/0.6/0.8` from one temporary target snapshot, leaves partial/custom
  schedules and other pairs untouched, and contains no order-table update.
- The backend settlement implementation remains unchanged. A focused unit test
  confirms `100 + 100 * 0.4 = 140`, while an existing order snapshot containing
  `1.4` still remains `1.4` and follows the historical snapshot calculation.
- Mobile and Admin now name the field as net profit excluding principal. Mobile
  regression coverage confirms it renders `0.4` as `40%` and does not subtract
  one from a historical `1.4` snapshot.
- Rust formatting, focused unit and migration tests, Admin focused tests,
  lint/typecheck, and the complete Mobile release gate passed. The fixture-backed
  migration case ran against real local MySQL (2/2, non-skipped), and the
  compiled migration binary applied the complete 0001–0125 chain twice in a
  fresh isolated database (122 successful records, final version 125, no dirty
  rows); all temporary databases were removed. No online write, deployment,
  commit, or push occurred.
