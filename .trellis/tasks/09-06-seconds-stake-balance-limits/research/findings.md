# Findings

- `mobile/src/views/SecondsView.vue`: amount begins as an empty ref but `load`, `selectProduct`, and `selectCycle` assign `cycleMin`. Wallet loading and `stakeAssetId` matching already exist; its sole balance hint is `sr-only`. Global loading/error and private reconciliation already guard asynchronous results.
- The selected `cycleLimitLabel` already derives both configured bounds, but fixed 26px/ellipsis clips the value. The console is 202px inside a fixed 420px operation grid; both need auto-sized tracks for balance and wrapping text.
- `mapSecondsCycle` maps backend `max_stake: null` to `maxStakeText: null`. `cycleHasMaximum` incorrectly treats this as a present maximum, causing valid unlimited cycles to render `--` and fail exact-range validation. Null is authoritative; malformed or number-only limits still fail closed.
- Keep exact financial arithmetic in `secondsFinancial.ts`. Do not introduce numeric comparisons, wallet fallbacks, or changes to immutable order-review/idempotency/reconciliation logic.
- Existing source geometry assertions live in `seconds-pencil-selected-parity`, `award-ui-trading-workspaces`, and `pencil-trading-product-selected-parity`; update only the requested Seconds layout contracts. Keep `SecondsView.vue` within its existing 3464-line/100942-byte budget without raising limits.
