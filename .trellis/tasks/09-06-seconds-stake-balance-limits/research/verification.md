# Verification results

## Automated gates

- Added four focused stake-form tests. Before implementation, the localized bounds, nullable maximum, and input/visible-balance regressions failed; existing exact-wallet behavior passed.
- Targeted financial and trading layout suite: 36/36 passed.
- Final `npm --prefix mobile run release:gate`: 680/680 tests, production and test type checks, PWA/Tauri builds (2148 modules each), artifact assertions, bundle budgets, source-size budgets, and critical-test behavior budgets passed.
- Fixed an initially duplicated existing locale key and updated the obsolete hidden-balance source assertion before the final successful release gate.
- `SecondsView.vue`: 3456/3464 lines and 99723/100942 bytes; no budget increases.
- Trellis task context validation, source integrity gate, and `git diff --check` passed.

## Browser setup and behavior

Ego task space 14 used the actual production Vue source via loopback Vite (13037), with a separate read-only API fixture (18087). No production API, user funds, database, or real orders were used. The fixture rejects order POSTs and recorded exactly zero attempted order creations.

- Initial/reloaded stake is empty even when the first cycle minimum is 500.
- Blank remains blank after duration selection and after clearing.
- Entered `25.50` remains unchanged when switching from a 10-minimum cycle to a 500-minimum cycle and back; invalid state changes true then false with the selected range.
- Opening a 60-second review shows normalized 25.5 USDT; cancelling preserves manual input.
- Nullable-maximum 120-second duration displays explicit no-maximum copy. A 1210 USDT draft within the 1234.56 balance passes review, and the dialog was cancelled.
- Switching product clears the draft and matches wallets by stake asset id. BTC balance is 0 despite frozen 5 and locked 6; missing USDC wallet is `-- USDC`, not zero or a login warning.
- Available 1234.56 is displayed without adding frozen 300 or locked 777.
- A five-second wallet delay shows loading without a fabricated number; initial wallet HTTP 503 shows `-- USDT`; a guest shows login copy without previously authenticated funds.

## Responsive checks

A 390px Chinese/light screenshot was visually inspected. DOM geometry checks covered 320px/390px, Chinese/English, and light/dark with large exact input values. All eight cases had zero horizontal document overflow and no clipped limit/balance content. The submit button remained inside its operation grid; the orders workspace began after it. Narrow normal limits expand to 52px and retain an 11px value font, rather than truncating.

| Locale | Theme | Width | Long-limit row | Long-balance row | Overflow |
| --- | --- | ---: | ---: | ---: | ---: |
| zh-CN | light | 320 | 68 | 68 | 0 |
| zh-CN | light | 390 | 68 | 28 | 0 |
| zh-CN | dark | 320 | 68 | 68 | 0 |
| zh-CN | dark | 390 | 68 | 28 | 0 |
| en | light | 320 | 84 | 68 | 0 |
| en | light | 390 | 68 | 52 | 0 |
| en | dark | 320 | 84 | 68 | 0 |
| en | dark | 390 | 68 | 52 | 0 |

Later screenshot capture attempts timed out at the browser CDP layer. The functional and geometry assertions were rerun successfully without screenshot capture; no additional visual screenshot verification is claimed for those cases.

## Handoff

Browser space 14 was closed (`done: true`), both loopback listeners were stopped, and the temporary API fixture script was removed. Source changes are uncommitted; no push or deployment was performed. Leave the task active until the user handles the code handoff/commit.
