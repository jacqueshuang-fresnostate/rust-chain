# N02 Mobile Numeric Hardening

## Scope And Plan

Owner: mobile production and behavioral tests, Mobile backend integration spec.
No backend/Web/PC, shared indexes, task metadata or PROGRESS edits. Main owns
the final progress entry. Preserve pre-existing dirty work.

1. Inventory production numeric sites by money, display, identity, time and geometry.
2. Repair DecimalText loss in convert, earn, loan and prediction adapters/views.
3. Bound decimal parser/arithmetic resources; fail closed for unsafe IDs/counts/epochs.
4. Exercise real adapters and shared transport, run release:gate and local browser checks.
5. Record intentional approximations and exact verification limitations here.

Shared API coordination: no wire shape migration required. Money is already
serialized as decimal strings; numeric IDs remain numeric and are rejected above
MAX_SAFE_INTEGER. Supporting larger IDs requires a separately coordinated server
string-ID migration; Mobile must not send a rounded ID in the meantime.

## Status

Production implementation and final verification complete. Final post-layout
`release:gate` process 90306 exited 0: 785/785 tests, zero failures/skips, both
build modes, artifact assertions and every budget passed. Browser process 73132
completed 24/24 persisted checks. No gate was waived or weakened.

## Delivered Changes

- Convert, Earn, Loan and Prediction response amounts, fees, rates and limits
  retain original decimal strings. Both integer-sized and fractional JSON
  numbers are rejected by their financial adapters; `String(number)` is not a
  financial compatibility path.
- Resource-bounded decimal parsing/arithmetic rejects oversized text, forged
  brands, extreme scales/exponents and unsafe Number fallbacks. Existing
  positive source-input precision remains 20 integer / 18 fractional digits.
  General helper bounds are 100/100 digits to support intermediate arithmetic;
  those bounds do not grant extra storage precision.
- IDs, pagination, product terms, asset precision and wire timestamps are
  checked safe integers. Timestamp multiplication is range checked. Shared
  Axios and private WebSocket ingress reject unsafe numeric JSON tokens.
- Invalid supplied limits no longer turn into unlimited capacity; missing
  wallet exact text cannot fall back to rounded display balances. Trade, Swap,
  Earn, Loan, Prediction, withdrawal and asset transfer use exact balances.
- Wallet available/frozen/locked buckets are summed exactly before formatting;
  non-USDT asset estimates require the exact ticker price. No independent
  bucket rounding or historical balance changes.
- Quote/bill/repayment and margin-close confirmations retain exact source
  digits. General formatting preserves tiny nonzero values. Margin-close
  percentages remain integer 1..100; previews use exact decimal ratios and
  can show 1e-20 for 1% of 1e-18. That preview is not a settlement promise.
- Browser-discovered long-value layout fixes cover Swap, Earn, Loan,
  Prediction, margin close, Assets and Home. Prediction's inner dialog overflow
  was measured separately from document overflow; both must be zero.

## Numeric Inventory

Scanned all production `mobile/src` with:

```sh
rg -n 'Number\(|parseFloat\(|parseInt\(|\.toFixed\(|\.toPrecision\(|Math\.(round|floor|ceil|trunc|pow)|v-model\.number|valueAsNumber|asNumber\(' mobile/src
rg -n 'Text \?\?|decimalTextFromBoundary|decimalTextFromFiniteNumber' mobile/src
```

This is a semantic classification, not a requirement that every Number call
disappear. Stringifying every ID/slider/chart field would change existing APIs.

| Class | Sites | Final contract / intentional numeric use |
| --- | --- | --- |
| Authority: money | `api/{swap,earn,loan,prediction}.ts`, `core/swapAssetLogos.ts` | String-only DecimalText, including product bounds, fees, rates, orders and bills. |
| Authority: existing financial Text models | `api/{wallet,trading,seconds,newCoin}.ts`, `core/{secondsOrder,newCoinModel,tradeFinancial,secondsFinancial}.ts` | Preserve exact Text fields; numeric companions remain compatibility display only. Actual mutation adapters normalize original text, never recovered Numbers. |
| Authority: validation/calculation | `core/{decimal,tradeForm,marginOrderConfirmation,withdrawalQuote,walletAmounts}.ts` | Bounded BigInt decimal arithmetic; safe scale/precision; invalid limits fail closed; wallet totals require exact buckets. `decimalPortion` uses safe integer UI ratios. |
| Authority: views | `views/{Trade,Swap,Earn,Loan,Prediction,Assets,Withdraw,NewCoinDetail,NewCoinRecords}.vue` | Text balance/input/limit authority. New Coin already required Text balances. Removed numeric fallbacks from other financial views. |
| Exact confirmation | `components/MarginCloseSheet.vue`, `core/marginClose.ts`, `views/{Trade,Orders}.vue` | Exact mark/quantity/PnL Text props and decimal percentage preview; the server still owns actual close amount. |
| Display: money | `core/format.ts`, `views/{Home,Assets,SecondsHistory,WithdrawalRecords}.vue` | Exact formatting where source exists. Compact notation intentionally approximates; normal money display caps do not change source values. Tiny nonzero is visible, not zero. |
| Display: compatibility companions | `api/{wallet,trading,seconds}.ts`, `core/{secondsOrder,tradeFinancial,tradeForm,marginOrderConfirmation}.ts` | `decimalDisplayNumber`/legacy numeric props remain for compatibility and presentation. Exact siblings control execution/confirmations. |
| Display: risk estimates | `core/marginRiskMetrics.ts`, `components/ContractTradeSheets.vue` | Finite read-only risk/strong-liquidation estimates and display widgets remain Number; server/exact snapshots remain authority. No local risk estimate becomes a wallet delta. |
| Display: unused legacy preview APIs | `core/{withdrawalQuote,marginClose}.ts` | Legacy Number preview exports retained for compatibility/tests; current production withdrawal and close components use the Text variants. |
| Bounded compatibility: leverage | `core/marginLeverage.ts`, `api/trading.ts` | Number UI shape preserved. Positive finite leverage must round-trip from decimal text exactly and stay below MAX_SAFE_INTEGER; long precision text and overflowing values fail. Local UI leverage/percentage is not a money JSON field. |
| IDs/counts | `core/numeric.ts`, `api/{wallet,trading,market,news,user,seconds,earn,loan,prediction,newCoin}.ts`, `core/{marketMapper,marketFavoriteMapper,newCoinModel,secondsOrder,walletLedger}.ts` | Safe integer/range checks. Existing opaque identity strings stay strings. Count/ID overflow cannot silently select another entity. Wallet submission's retained `asNumber(id)` is immediately checked with `Number.isSafeInteger`. |
| Route ID | `views/NewsDetailView.vue` | Route Number conversion is immediately guarded by positive safe-integer check before loading. Navigation `core/navigation.ts`/`components/PageHeader.vue` numbers are local route-depth metadata, not entity IDs. |
| Wire time | `core/{numeric,realizedReturn,marketTickerFreshness,marketChart}.ts`, financial adapters, `api/marketSocketProtocol.ts` | Safe integer epochs and JS Date range; checked seconds-to-ms conversion. Historical REST compatibility remains, malformed/fractional/unsafe values are rejected. |
| Local time/display | `core/{returnHistory,predictionLocale,homeMarketBrief,supportChat}.ts`, `components/MarketProvenanceLabel.vue`, New Coin cards/detail | UTC day grouping, validated timestamp display, countdown rounding, and local chat nonce generation. Not server-side expiry or settlement. |
| Local lifecycle scheduling | `api/{privateUserStream,marketTickerStream,marketDetailStream}.ts`, `core/{privateUserStreamManager,sessionOwner,marketLifecycle}.ts`, `pwa/{index,eligibility,update}.ts` | Existing local Date.now/session generations, retry jitter, timer defaults and install-nag times remain numeric. Not financial DTOs or server expiry policy; arbitrary injected clocks/timer options were not comprehensively reworked in N02. |
| Chart/presentation | `api/marketSocketProtocol.ts`, `core/{marketMapper,marketChart,marketChartRuntime,marketTickerFreshness,homeMarketBrief}.ts`, `components/OrderBookPanel.vue`, `views/{Markets,Seconds,Trade}.vue` | Finite OHLCV/depth/market-percent data, candle seconds, MA/pixel geometry. Ticker authority requires lastPriceText; chart close is not an execution fallback. |
| Bounded geometry | `core/{decimal,newCoinPresentation,returnHistoryGeometry,slideToConfirm,marketChartTheme,performanceTier}.ts`, `components/{SignalField,MarginCloseSheet,ContractTradeSheets}.vue` | 0..1 decimal ratio conversion, slider percentages, canvas DPR/sizing, RGB parsing and device capability buckets. |
| UI counts/booleans | `views/{Kyc,Loan,Security,Profile}.vue`, `core/apiError.ts` | File-size labels, held-asset sort booleans, security completion count and HTTP status. No monetary authority. |

Residual boundary explicitly retained: fractional local leverage widgets and
read-only numeric risk/chart companions still use IEEE-754. The shared JSON
guard cannot recover precision already lost in a fractional JSON number; this
is why financial string-only adapters, not that generic guard, establish money
authority. A future numeric-ID-to-string migration and native timer/config
hardening are separate compatibility work, not silently claimed here.

## Changed Paths

N02 production additions:
`mobile/src/core/numeric.ts`, `mobile/src/core/walletAmounts.ts`,
`mobile/src/styles/home-numeric.css` (shared-style long-value override; Home
retains its existing no-scoped-style contract).

N02 production edits (some already contained unrelated dirty work):

- `mobile/src/api/{earn,loan,market,marketSocketProtocol,newCoin,news,prediction,privateUserStream,seconds,swap,trading,user,wallet}.ts`
- `mobile/src/core/{apiRequest,decimal,format,marginClose,marginLeverage,marginOrderConfirmation,marketChart,marketFavoriteMapper,marketMapper,marketTickerFreshness,newCoinModel,realizedReturn,secondsOrder,swapAssetLogos,tradeForm,withdrawalQuote}.ts`
- `mobile/src/components/MarginCloseSheet.vue`
- `mobile/src/views/{Assets,Earn,Home,Loan,Orders,Prediction,SecondsHistory,Swap,Trade,Withdraw,WithdrawalRecords}View.vue`
- `.trellis/spec/mobile/backend-integration.md`

N02 behavioral regression and reproducible browser fixture:
`mobile/tests/numeric-safety.test.ts`,
`mobile/tests/fixtures/numeric-browser.{html,mjs}`.

Supplemental existing assertions/fixtures updated:
`mobile/tests/{android-ui-foundation-slice-a,award-ui-assets-profile,margin-close-sheet,margin-product-boundaries,market-kline-history-api,pencil-selected-page-parity-20260807,priority-secondary-page-parity,secondary-product-order-views,seconds-history-view,spot-trading-ui-optimization,swap-asset-logos,trading-lending-views,transaction-records-pencil-parity}.test.ts`.
Pre-existing chart provenance/idempotency/refund and other dirty work was
preserved; the complete working-tree diff is not an N02-only change list.

## Verification And Browser Evidence

- `numeric-safety.test.ts` runs real Vite SSR adapters with only HTTP replaced.
  It verifies >2^53 and 1e-18 values, exact payloads and quote IDs, fractional
  JSON numbers, invalid/negative/nonfinite/huge decimals, 19 fraction/21 integer
  rejection, trailing zeros, unsafe IDs, transport interception, bucket sums,
  full confirmation formatting and 1% margin-close dust.
- Final gate: `npm --prefix mobile run release:gate`, exit 0. Type checks,
  785/785 tests (0 failed, 0 skipped), PWA/Tauri builds and both artifact/bundle
  checks, source-size and critical-behavior budgets all passed. No budgets raised.
  Full log: `mobile/tests/evidence/n02/release-gate.log`.
- Final `git diff --check -- mobile .trellis/spec/mobile/backend-integration.md
  .trellis/tasks/09-19-numeric-safety-hardening/research/n02-mobile.md` passed.
- Browser: one ego-browser TaskSpace 30, local Vite port 5197, actual App and
  actual production SFCs, fake local authentication, Axios/WebSocket fixtures.
  No production requests or actual financial commits. Remote/API network paths
  additionally blocked in the browser during verification.
- Viewports: 320x900, 390x900 and 1440x900. At desktop, the existing application
  intentionally retains its mobile-width canvas alongside its desktop shell.
- Swap: typed `9007199254740993.000000000000000001`, requested the local quote,
  opened confirmation, verified exact request text, full source amount and
  1e-18 target/fee. No confirm mutation was sent.
- Loan: opened the existing loan's repayment confirmation and verified the
  full repayment/principal text at all three widths.
- Earn: opened product subscription, filled the large amount, checked exact
  textbox/balance, tiny APR and explicit `<0.01` daily-yield/threshold display.
  Product/holding rows wrap long amounts without clipping.
- Prediction: selected Yes, typed the large amount, requested a local quote,
  checked exact shares/payout and 1e-18 fee. The final document AND dialog
  horizontal overflow measurements are zero at all three widths.
- Margin close: mounted the actual component with exact risk props, used its
  real range keyboard Home action to select 1%, checked exact large quantity
  and +1e-20 preview, document/dialog overflow zero. Full Trade/Orders route
  settlement was not exercised in this component fixture.
- Assets/Home: exact bucket-based large total is preserved, long balance labels
  fit/wrap, document overflow zero. Mock return-history/today-return endpoints
  deliberately lack responses and show unavailable, not invented zero return.
- Screenshot files: `mobile/tests/evidence/n02/n02-{swap,loan,earn,earn-list,prediction,margin-close,assets,home}-{320,390,1440}.png`.
  Screenshots were visually inspected, including the 320px cases that exposed
  layout problems despite a zero outer-document overflow measurement.
- Machine-readable final DOM/requests/viewport evidence:
  `mobile/tests/evidence/n02/browser-verification.json`. All 24 document/dialog
  overflow pairs are zero; exact-value assertions ran before each screenshot.
  Raw production conversion scan:
  `mobile/tests/evidence/n02/numeric-sites.txt`. Artifacts total approximately
  4.5 MB; no credentials or production responses are included.
- Cleanup complete: TaskSpace 30 finished with no retained tabs; Vite session
  12240 stopped intentionally with SIGINT (exit 130). `lsof -nP -iTCP:5197
  -sTCP:LISTEN` returned no listener. All N02 verification sessions completed;
  shared DB/Redis and the original 5178 service were not operated by N02.

## Exact Limitations

- No production operations, deployment, commits, database use, backend/frontend
  integration against a real wallet, real deposits/withdrawals, or native device
  execution. Tauri gate means frontend Tauri-mode build/artifact verification,
  not a signed iOS/Android build or native WebView test.
- Browser coverage above is the listed light-theme local fixtures, not every
  product state, every theme, all locales or every changed secondary history
  route. Withdraw/Seconds history/record wiring is covered by the full tests,
  not a new browser end-to-end run.
- IDs beyond MAX_SAFE_INTEGER intentionally fail closed; full large-ID support
  needs a shared API migration. Rates/asset settlement precision and actual
  generated-amount quantization still belong to backend product rules.
- General Money display may round ordinary summaries; exact confirmation
  values and source models remain intact. Editable input/select controls may
  scroll/clip their text internally as native controls do; their value is not
  shortened. The duplicate slider-button amount may ellipsize, with the exact
  value shown in the adjacent confirmation rows and its title.
- Private/local timer option normalization is not claimed as comprehensive
  configuration hardening. Financial wire IDs/counts/timestamps are guarded;
  backend JWT/config/time hardening belongs to Main's shared slice.
- Main alone updates PROGRESS/task metadata/indexes. No changes made there.
