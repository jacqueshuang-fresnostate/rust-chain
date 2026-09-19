# E04 Ticker And Candle Continuation

## Contract

- Ticker/cache/event/REST and candle/cache/event/Mongo records retain producer
  provenance and actual observation time. Legacy absence stays unknown.
  Shared Strategy provider is generated, not guessed strategy/default subtype.
- Redis candle sequence/CAS is unchanged; additive observation is displayed,
  not substituted into execution or settlement prices.
- PC numeric compatibility candle arrays retain evidence through both chart
  adapters. Charts summarize all observed categories, including unknown.
- PC store no longer drops REST evidence, lets late REST overwrite newer WS
  prices, or inherits an old provider on an unknown-source live update.
- Mobile keeps source observation separate from local receipt/order timestamps.
  Missing source time displays unknown; old/future evidence displays stale.
  Live source/time and price move together through late snapshot races.

## Verified Slice

- Rust producer/cache/REST/WS/Mongo provenance tests: 7/7.
- PC chart/adapter/store focused tests: 7/7; type-check and build passed.
- Mobile final slice release gate: 777/777 plus app/test type-check, PWA and
  Tauri builds/artifact checks, bundle, source and behavior budgets passed.
  Actual SFC tests include mixed history, freshness timer, future/missing time,
  source changes and unmount; existing history I/O helper now renders the real
  provenance component rather than ignoring the import.
- Local-only Ego Browser TaskSpace14: Mobile320px and PC1440px/390px render
  actual components with fixture candles. Unknown/fresh/stale switching and
  source summaries visible; no document horizontal overflow. Mobile and PC
  Lightweight canvas pixel checks passed. KlineCharts screenshot confirms
  nonblank rendering (initial pixel sample preceded its asynchronous paint).
  This is not live market or authenticated backend E2E.
- Screenshots: `/tmp/e04-ticker-mobile-320.png`,
  `/tmp/e04-ticker-pc-1440.png`, `/tmp/e04-ticker-pc-390.png`,
  `/tmp/e04-ticker-pc-klinecharts-verified.png`.
  Temporary HTML fixtures deleted, task space finished, test servers stopped.
- Initial full PC test run: 101/106; five API/WS-origin expectations assumed an
  unconfigured localhost default. Final full run: 106/106 after explicitly
  setting/restoring the mocked origin in test fixtures, preserving exact route,
  token encoding and subscription assertions. API test additionally verifies
  custom HTTPS origin/prefix. No production origin or WS behavior was changed.
- Actual isolated Redis regression: 5/5 passed, including equal-observation
  replay fences, concurrent ticker writers and candle observation ordering.
  New later refund/frontend edits require another final integration gate.

## Paths

Backend market presentation/repository/cache/feed, market unit tests and Redis
cache test; PC adapters/types/store, `klineData`, both chart engines,
`MarketChart`, ticker label/Trade binding; Mobile ticker mapper/freshness/types,
candle normalization, shared chart/label, chart hosts, locales, provenance and
SFC tests. Full trade/depth portion remains documented in `e04-delivery.md`.
