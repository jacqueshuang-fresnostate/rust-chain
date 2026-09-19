# E04 Market Provenance Delivery

Date: 2026-09-18. Scope: the dispatched trade/depth presentation sidecar.
The main agent owns integration, PROGRESS and task-wide acceptance.

## Delivered Contract

- Public REST depth and trade rows, and public depth/trade feed events, carry
  additive `source` and `provider` fields. Existing prices, quantities, timestamps,
  event identities, subscriptions and order-execution behavior are unchanged.
- Semantic sources are `platform`, `external`, `strategy`, `default`,
  `generated`, and `unknown`. Provider remains separate: `platform`,
  `bitget`, `htx`, `coinbase`, or the existing shared `strategy` provider.
- Only conversion of an existing `SpotTradeRecord` labels a row `platform`.
  External-market REST recent trades still read platform records, while their
  public WS prints can be external references. These are intentionally not
  relabeled as one homogeneous stream.
- The existing generator produces IDs
  `strategy:<positive-id>:v<positive-version>:<event-second>` or
  `default:<positive-id>:v<positive-version>:<event-second>`.
  Provider `strategy` plus that validated ID/time convention proves the
  subtype. An external provider with the same-looking ID remains external.
  Unrecognized generated IDs stay generic `generated`.
- A generated depth frame uses the matching, already-validated same-frame print
  evidence when present, consistently in Redis, REST and WS. Without a print
  it remains `generated`; no current pair configuration or current price is
  consulted to guess historical provenance.
- The depth cache adapter now saves provenance. Old cache JSON without evidence
  returns `source=unknown`, `provider=null`, rather than fabricating a provider.
  No cache-key, Lua ordering, ticker fencing, or financial data change was made.
- Both clients retain evidence during REST/WS normalization and rendering.
  Trade deduplication/render keys include provider/source, so equal numeric IDs
  from platform and external feeds coexist. Mobile frame coalescing and
  independent snapshot loading retain evidence, including empty live books.
  PC keeps live prints ahead of late REST and rejects obsolete symbol responses;
  depth metadata follows the same owner and live-over-REST rules as its rows.
- Visible localized labels: 平台成交, 外部参考, 策略生成（模拟）,
  默认生成（模拟）, 平台生成（模拟）, 来源未知; English equivalents are supplied.
  Raw source/provider remain available in the label title. Missing, unsupported,
  malformed or contradictory evidence never becomes a platform execution label.
- Mobile labels cover all `OrderBookPanel` layouts and MarketDetail/Trade prints.
  PC labels cover the shared spot/margin book and shared MarketTrades component.
  The Mobile trade row was extracted without changing price/quantity formatting,
  to stay under the existing source-size budget, not to raise that budget.
  The existing Pencil fingerprint normalizes only the exact approved provenance
  bindings/row extraction; its original digest is unchanged.

## Files Changed By This Sidecar

Backend and backend tests:

- `src/modules/market/presentation.rs`
- `src/modules/market/infrastructure/cache.rs`
- `src/modules/market/infrastructure/adapters/feed.rs`
- `src/modules/market/infrastructure/adapters/ingestion.rs`
- `tests/unit_src/src_modules_market_mod_tests.rs`

Mobile:

- `mobile/src/api/market.ts`
- `mobile/src/api/marketDetailSnapshot.ts`
- `mobile/src/api/marketDetailStream.ts`
- `mobile/src/api/marketSocketProtocol.ts`
- `mobile/src/core/types.ts`
- `mobile/src/core/marketProvenance.ts` (new)
- `mobile/src/components/OrderBookPanel.vue`
- `mobile/src/components/MarketProvenanceLabel.vue` (new)
- `mobile/src/components/MarketTradeRow.vue` (new)
- `mobile/src/views/MarketDetailView.vue`
- `mobile/src/views/TradeView.vue`
- `mobile/src/i18n/messages/zh-CN.ts`
- `mobile/src/i18n/messages/en.ts`
- `mobile/tests/market-provenance.test.ts` (new)
- `mobile/tests/market-detail-stream.test.ts`
- `mobile/tests/market-snapshot-loading.test.ts`
- `mobile/tests/market-socket.test.ts`
- `mobile/tests/pencil-trading-product-selected-parity.test.ts`

PC:

- `pc/src/api/backendAdapters.ts`
- `pc/src/api/marketProvenance.ts` (new)
- `pc/src/components/trade/MarketTrades.vue`
- `pc/src/components/trade/OrderBook.vue`
- `pc/src/components/trade/MarketProvenanceLabel.vue` (new)
- `pc/src/views/Trade.vue`
- `pc/src/views/Contract.vue`
- `pc/src/i18n/index.ts`
- `pc/tests/market-provenance.test.ts` (new)
- `pc/tests/stomp.test.ts`

Delivery record: this file. No PROGRESS, global specs, migrations, navigation,
spot execution, wallet/risk, Admin governance, production or git commit changes
were made by this sidecar. Pre-existing and concurrent unrelated edits remain.
`events/service/websocket.rs` was inspected, not edited: its existing
`from_market_feed_event` already serializes the complete payload unchanged.

## Verification

Run from the repository root unless stated otherwise:

| Command | Result |
| --- | --- |
| `cargo test --lib market -- --nocapture` | 73/73 passed, including all five new provenance tests. |
| `cargo test --test backend_architecture --test backend_documentation -- --nocapture` | 11/11 architecture and 1/1 documentation passed. |
| `rustfmt --edition 2024 --check --config skip_children=true src/modules/market/presentation.rs src/modules/market/infrastructure/cache.rs src/modules/market/infrastructure/adapters/feed.rs src/modules/market/infrastructure/adapters/ingestion.rs tests/unit_src/src_modules_market_mod_tests.rs` | Passed on all owned Rust files. |
| `npm --prefix mobile run release:gate` | Passed: production/test types, 772/772 tests, PWA and Tauri web builds, artifact assertions, both bundle budgets, source-size and test-quality gates. Native Android/iOS binaries were not built. |
| `node --experimental-strip-types --test --test-reporter=spec tests/*.test.ts` (cwd `mobile`) | 772/772 passed independently before the release gate. |
| `npm --prefix pc run type-check` | Passed. |
| `npm --prefix pc run build` | Passed; existing stale Browserslist database advisory only. |
| `node --experimental-strip-types --test pc/tests/market-provenance.test.ts` | 3/3 passed. |
| `node --experimental-strip-types --test --test-name-pattern='depth\|public trade protocol' pc/tests/stomp.test.ts` | 2/2 focused public protocol tests passed. |
| `node --experimental-strip-types --test --test-reporter=spec pc/tests/stomp.test.ts` | 8/12. Four existing URL assertions expect `ws://127.0.0.1:8080` while unchanged `APP_CONFIG` selects `wss://hipoex.cllbmz.kdns.fr`; no real sockets are opened by these mocked tests. Not fixed outside E04 scope. |
| `cargo fmt --all -- --check` | Failed on concurrently edited, non-E04 convert/loan/OpenAPI/prediction/seconds/wallet files. Did not format others' work. Owned-file check above passed. |
| `git diff --check` | Passed. |

An initial Rust attempt hit the other agent's temporarily missing
`tests/unit_src/src_modules_admin_financial_retries_tests.rs`; after that
slice became available, the final commands above compiled and passed.
Initial Mobile execution from the repository root exposed cwd-dependent tests;
the correctly scoped Mobile run and full release gate above passed.

## Browser Evidence

Ego Browser TaskSpace 8 used temporary local-only component fixtures at
`http://127.0.0.1:17111/e04-preview.html` (Mobile) and
`http://127.0.0.1:17112/e04-preview.html` (PC).
The fixtures mounted the actual production OrderBook/MarketTrades/MarketTradeRow
and label components with local deterministic market payloads; PC HTTP and
subscription callbacks were mocked, not connected to a backend.

- Mobile: 320, 390 and 448 CSS px, Chinese/English, zero horizontal overflow,
  no clipped labels. English generator labels wrap and rows grow to 41px.
  Separate 320px light/dark screenshots used the production theme tokens.
- PC: 1440px, Chinese/English, all six labels readable and no horizontal overflow.
  A live external print coexisted with platform REST prints; switching book
  evidence to unknown immediately displayed 来源未知.
- Inspected screenshots: `/private/tmp/e04-pc-1440.png`,
  `/private/tmp/e04-mobile-light-320.png`,
  `/private/tmp/e04-mobile-dark-320.png`.
- Commands used: `npm run dev -- --host 127.0.0.1 --port 17111` in `mobile`,
  `npm run dev -- --host 127.0.0.1 --port 17112 --strictPort` in `pc`,
  and `ego-browser nodejs -e ...` using TaskSpace 8, CDP viewport changes,
  DOM clipping checks and screenshots.
- Both temporary fixture files were deleted with apply_patch, the TaskSpace
  finished, and both local Vite processes stopped. No production operation.

## Exact Remaining Gaps

1. **Not full ticker/K-line provenance.** Ticker caches/REST still omit provider.
   K-line history/read DTOs still omit stored source, and chart adapters do not
   carry per-candle provenance. Existing WS ticker/K-line provider fields and
   Mobile pair-level generated-market notice remain unchanged. Do not mark
   universal E04 acceptance complete based only on this trade/depth slice.
2. **Zero-volume/legacy generated depth.** Both generators share the Strategy
   domain provider. Without a matching subtype-bearing print there is no
   subtype evidence on the detail frame, so the honest label is generic
   平台生成（模拟）, not an invented default/manual distinction. Richer depth
   provenance would require producer/domain evidence outside this ownership.
3. **Legacy depth cache.** Previously written JSON has already lost provider.
   It stays unknown until a fresh producer write; no speculative backfill.
4. **Platform trade direction.** Existing REST platform conversion returns BUY
   irrespective of aggressor side. This slice labels the record's origin only;
   it does not certify direction or change matching/manual-fill semantics.
5. No live MySQL/Redis/Mongo HTTP+WS integration, provider network test, native
   device test, complete application-page browser E2E, PC full-suite green,
   or production deployment was claimed. Browser evidence is actual-component
   rendering with mocked transports, complemented by protocol and owner-race
   tests. The main agent should rerun task-wide Rust fmt/clippy/check after all
   concurrent slices have settled.
