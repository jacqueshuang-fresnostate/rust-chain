# Coinbase REST fallback request-time window

## Scope and implementation

Only the Coinbase candle fallback URL materialization path changed. External
K-line CAS/history/publication defects and provider 4h mapping remain deferred
in `backend-market-query-audit.md`; this slice does not weaken or remove any
cache/history guard.

- `src/modules/market/infrastructure/adapters/provider.rs` adds a pure
  `coinbase_rest_kline_url_at(configured_url, interval, now)` helper. It reuses
  the existing interval-to-seconds mapping, keeps the existing 300-candle
  window and saturating subtraction, and replaces all start/end query pairs
  with one pair each using the supplied UTC timestamp in seconds. Existing
  endpoint/port/path, granularity, custom query values (including repeated
  keys), and fragments are retained using the existing `url` dependency.
- `src/modules/market/infrastructure/adapters/feed.rs` calls the helper
  immediately before **each** Coinbase K-line HTTP dispatch, not when its
  generation/config is created and not just once at the start of a serial
  request batch. Only a request-local URL is updated; long-lived config
  objects and their diagnostic preview URL getters stay immutable.
- The private fetch implementation accepts an explicit clock closure for
  deterministic dispatch tests; the production wrapper supplies `Utc::now`.
  No dependency or user-facing clock setting was added.
- Bitget/HTX requests and Coinbase ticker requests bypass materialization and
  retain their exact original URL bytes. Timeout clients, proxy settings,
  provider generation fencing, retry ordering, payload wrapping, ingestion,
  broadcast and financial behavior are unchanged.
- A malformed configured URL becomes a normal isolated request failure;
  later requests still run. A refreshed request that fails HTTP records the
  **actual attempted URL**, not the old config window, in its failure context.

## Tests / red evidence

Added an actual dispatch regression to `tests/market_feed_worker.rs` before
implementation. It constructs a Coinbase request with an explicit old time
window and a custom proxy path/query, invokes the real fallback worker with a
recording (no-network) client, and inspects the URL actually passed to that
client. Original behavior failed with the unchanged
`start=1710000000&end=1710090000` URL:

`cargo test --test market_feed_worker
coinbase_rest_fallback_refreshes_old_window_at_actual_dispatch -- --nocapture`
-> **failed**, 0 passed / 1 failed. This was an executed red regression.

The existing Coinbase success fixture was changed locally to capture dynamic
URLs and explicitly assert the stable endpoint/granularity/window plus unchanged
ingested events, rather than index by a full URL frozen at config creation.
The existing exact-URL recording client used by all other provider tests is
unchanged; the Coinbase test no longer depends on both operations happening
within the same wall-clock second.

`tests/unit_src/src_modules_market_infrastructure_adapters_feed_tests.rs`
contains five standalone tests (the production file only declares the module):

1. Fixed T/T+12h URL materialization; proxy path, fragment, custom/duplicate
   query values remain; duplicate start/end pairs are collapsed.
2. Existing 300-candle widths for 1m/5m/15m/1h/1d and explicit preserved legacy
   4h mapping; near-epoch subtraction semantics and malformed URL rejection.
3. Two serial requests use distinct injected dispatch times; the first HTTP
   failure is isolated and its recorded URL is the refreshed URL.
4. Invalid URL is not dispatched and does not prevent the later valid request.
5. Bitget, HTX and Coinbase ticker URLs are byte-identical and never consult the
   rolling-window clock.

## Verification status

- Scoped `rustfmt --check --edition 2024` and `git diff --check` passed after
  source/test edits.
- Focused green Cargo verification completed sequentially after
  `risk_rule_hardening` confirmed its production files were stable:
  - `cargo test --offline --lib
    modules::market::infrastructure::adapters::feed::tests:: -- --nocapture`
    -> **5 passed**, including deterministic per-request dispatch times and
    actual refreshed failure-context URL assertions.
  - `cargo test --offline --test market_feed_worker -- --nocapture`
    -> **33 passed**, including the prior red stale-window regression and
    existing provider/runtime/failure-isolation/timeout contracts.
  - `cargo test --offline --test market_adapters -- --nocapture`
    -> **5 passed**, preserving existing Bitget, HTX and Coinbase parsing.
  Cargo was then released to the risk worker; root owns all-target gates.
- No external provider, local database, deployed service, Git commit/push,
  specification or PROGRESS edit was performed by this worker.
