# Mobile snapshot ownership and loading

## Evidence and defects

Both MarketDetailView.load and TradeView.loadMarketData originally awaited
Promise.allSettled for K-lines, depth and trades before committing any result.
Ready history could remain invisible until a slow book/trades response. Trade's
interval refresh also incremented the full-load generation, discarding otherwise
valid symbol-owned book/trades and leaving loading true without a live depth
frame. Live K-line callbacks cleared chartLoading before initial history settled,
preventing the renderer from identifying history hydration after several live bars.

The source-extracted, executable view regression initially produced 7 failures
out of 10; 3 passed. It runs production loader functions/callbacks and the real
stream session against deferred API I/O. The stopped-view harness models the
viewActive guard and session stop; the old detail view also incremented its
version on real unmount, so that particular red result is guard-hardening
evidence rather than a demonstrated production unmount defect.

## Implementation

- New api/marketDetailSnapshot.ts starts all three requests concurrently and
  commits each independently. Transport failure is caught separately from UI
  callback execution. K-lines require the current session's request generation;
  depth/trades require active-view, symbol and full-load generation only.
- Interval changes replace the stream/K-line generation, not the symbol/load
  generation. Full reload, symbol change and unmount still invalidate old work.
- Per-load liveDepthReceived survives any number of interval changes. A valid
  empty live book is authoritative too, and late REST cannot resurrect stale rows.
- Separate depth/trades loading flags drive their own panels. A live frame may
  settle its panel, but only the current REST history settlement clears
  chartLoading. Visible live candles remain visible during hydration.
- Existing mergeMarketTradeHistory and session.resolveKlineRequest retain
  live-over-REST authority; no OHLC, socket protocol, fetch URL, interval or price
  calculation changed. Existing API timeouts remain the request bound.

## Coverage and constraints

21 tests cover the two actual views: independent channel completion, interval
switch, visible live data during failed history loading, stale REST/live races,
multiple interval changes with an empty book, stop, symbol switch/same-symbol
retry, failure isolation and per-panel template bindings. The stream suite adds
13 existing session/protocol tests: combined 34/34 pass. Renderer worker
independently executed 6 further lifecycle cases against this harness.

Source-only tests were narrowly updated from live reactive symbol arguments to
captured symbol closures and the shared loader wiring. The spot template hash
still guards its original digest; only the intentionally changed tradesLoading
binding is normalized back via replaceExactlyOnce. No budget was increased:
TradeView is 6131/6131 lines and 179598/179654 bytes.

Initial full Mobile release gate: 728/728 tests, production/test types, PWA/Tauri web
builds, artifact, bundle and governance checks passed. Two initial runs exposed
only stale source-pattern/digest assertions (5, then 1); the final full rerun
passed without lowering tests, timeout or budgets. Native binaries and deployed
network/production acceptance are outside this local slice.

A final period-transition check reproduced Trade retaining its previous-period
candles under the new selection (Detail already cleared). Trade now clears only
points immediately after stream replacement, preserving book/trades. New regression
was 1 pass / 1 fail before repair. This also matches the renderer's empty/new-key
hydration contract. The final full gate is rerun after this one-line fix; its
result supersedes the initial 728-test gate in review.md.
