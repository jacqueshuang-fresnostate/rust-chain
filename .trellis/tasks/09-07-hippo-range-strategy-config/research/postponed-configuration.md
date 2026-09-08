# Postponed HIPPO-USDT schedule — latest saved state

Recorded2026-09-07 04:45, Asia/Hong_Kong(UTC+08). This report supersedes the03:30–04:30 schedule for current-state lookup; earlier reports remain historical records.

## Request and applied change

User said it was already04:29 and requested postponement. Root announced a new05:00–06:00 single-hour window for2026-09-07. Cancelled the old pending enable dialog without confirming it. Loaded current authoritative configuration and verified paused status. Only the five business time fields were edited:

- start2026-09-07 05:00
- end2026-09-07 06:00
- node1 time05:15
- node2 time05:30
- node3 time05:45

All price/scenario/noise/volume/seed choices were preserved: start/target0.1,range,global/local volatility0.02,mean0.9,noise1.5,wick0.1,uniform volume200..60000,auto seed with regeneration unchecked,relative-to-start node targets+6/-4/+5,soft mode,tolerance1%,node volume overrides blank. The backend appends its ordinary new configuration version/checkpoint metadata; this is not a promise of identical minute-by-minute random values after retiming/versioning.

## Preview and saved readback

Actual UI preview: version4,60 total minutes and60 returned candles, high0.105877,low0.095670. Read all60 rendered OHLCV rows using read-only DOM APIs:

-33 up,27 down
-first05:00:open0.1,high0.100289,low0.099853,close0.100285,volume23049.4604
-last05:59:open0.100343,high0.100507,low0.099875,close0.1,volume16407.5342
-all positive lows,valid high/low envelopes,volume bounds200..60000,and next-open equals preceding-close

First save attempt returned UI HTTP403. No blind retry: cancelled the dialog, discarded only the documented unsaved form, performed normal logout/login, re-read stored dates and confirmed they were still03:30–04:30 with old node times. Re-entered only the five times; a new no-side-effect preview again returned version4 and identical60-candle extrema. The second save completed; reopening through the authoritative detail load verified05:00–06:00 and all three new times, with every non-time parameter and paused status unchanged. One successful saved postponement, preceded by one rejected attempt. Persisted version number was not separately queried from version history.

## Current handoff

Closed the saved editor without changes. Opened a fresh “启用行情策略” dialog and filled its reason with the new05:00–06:00 window. The confirmation button is enabled but was NOT clicked. Tab1 is marked for handoff. Strategy remains paused at the last authoritative read; user must perform final enablement. Existing fixed-hour expiry remains06:00; no recurring schedule,automatic extension,historical recovery,order/wallet operation,or activation was executed.

## HTTP403 diagnostic limits

Observed only the UI diagnostic HTTP_403, not the raw live response body/headers; it is not established that it was non-JSON or caused by JWT expiry. Normal re-login restored the action. A read-only local code audit found:

- Web client.ts:238–253 refreshes only401;403 is surfaced directly.
- client.ts:256–262 falls back to generic HTTP status for unrecognized/non-JSON error payloads.
- auth/mod.rs:409–419,498–505 maps ordinary JWT/Sa-Token expiry to Unauthorized.
- error.rs:71–74,131–147 emits JSON401/403 distinctly.
- config.rs:205–208 default access TTL900seconds; actual production TTL was not read.

Proxy/edge handling or deployment differences are hypotheses only. No security settings, auth implementation, token storage, or source code were modified.
