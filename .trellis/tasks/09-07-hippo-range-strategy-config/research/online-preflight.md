# HIPPO-USDT configuration preflight

## Online observations (2026-09-07, Asia/Hong_Kong)

The specified admin UI was accessed using user-supplied credentials. No credentials, auth/session tokens or account records are retained here. The production page contains exactly one strategy: strategy1, pair3, HIPPO-USDT, active. At the list load, latest tick was02:43:09 and the UI reported normal push. This is a loaded observation, not continuous health proof.

Current editor values:
- start/target price: both0.100000000000000000
- start:2026-09-06 04:27; end:2026-09-30 04:27
- global volatility:0.5 (50%)
- mean reversion:0.9; noise:1.15; wick:0.95; volume shape:uniform
- auto seed, regeneration unchecked (value deliberately not retained)
- volume bounds:200..60000 per minute, unchanged
- three soft nodes, tolerance1%, relative-to-start targets+6,-4,+5
- node dates:September12/18/24 at04:27; local volatility0.0115 each

This schedules a turning point only every six days. A one-hour 1m viewport is not equivalent to the full24-day oscillation. Local node volatility overrides global settings, so the global50% is not the active noise ratio before the first node. Existing raw candles and the screenshot's manual scale were not fetched; the old0.6 axis cannot be uniquely attributed to the current configuration.

## Draft and preview only

Changed the editor draft (not persisted): global volatility0.02, all three local volatility0.02, noise1.5, wick0.1. Mean0.9, price/time boundaries, relative node targets, tolerances, seed settings and volume unchanged.

The real no-side-effect preview returned version3,34560 minutes,120 non-continuous samples. UI sample high0.105978, sample low0.095984. First sample open0.100000,high0.100117,low0.099956,close0.100001. Those extrema describe only the120 returned samples, not a guaranteed bound over all minutes. Wide preview was not fully visible in the narrow browser screenshot; data and metrics were verified from the UI accessibility tree.

No pause, save, enable, rollback, history detection/recovery, order or wallet mutation was executed. Active strategy continues unchanged.

## Runtime preflight from existing backend source

- application/market.rs:972-1061: non-active-only config PATCH creates a new version every successful call; pause/edit/enable are separate operations.
- application/market.rs:1017-1039 and infrastructure/market.rs:523-547: version effective time is request start_time; run checkpoint/current_price reset from start_price.
- workers/synthetic_market.rs:388-403 and modules/market/synthetic.rs:700-711: runtime recalculates current minute from new snapshot; version participates in noise. Keeping seed is not a guarantee of price continuity.
- workers/synthetic_market.rs:281-355,471-495: no batch historical rewrite or automatic backfill; a newer same-minute observation may replace current OHLCV.
- modules/market/infrastructure/adapters/ingestion.rs:231-262: accepted ticker archives market_price_ticks and triggers spot/margin limit orders.
- modules/seconds_contract/infrastructure.rs:1069-1095: seconds settlement reads these ticks.

Thus final enablement is not merely a visual preference change and is reserved for user operation.

## Pending decision / handoff

Asked whether to shorten the original24-day schedule to a fresh1-hour or4-hour run or preserve its original expiry. One-hour timing in earlier advice was a preview example, not a sufficient reason to silently shorten the actual running strategy. No time boundary/node-time changes made pending that answer.

In-app browser tab1 remains at the unsaved editor plus no-side-effect preview and is marked for continuation. Do not reload/close without accepting that the draft will be lost. On resume, inspect fresh UI state; no duplicate login or unreviewed submission.
