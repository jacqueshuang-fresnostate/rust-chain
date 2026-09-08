# Reference evidence for BTC-following default generation

## Scope and conclusion

Read-only source research on 2026-09-07. No provider requests, database queries, credentials, production edits, or online configuration changes were made. Local source establishes available storage contracts, not that a particular deployment currently has fresh BTC data.

The follower can read already-ingested external observations from MySQL without making any new public-network request. The smallest compatible first step is a **one-minute-lagged, explicitly sampled reference return**, frozen with the target minute's deterministic state. Calling an arbitrary old Mongo candle a confirmed complete 1m candle is not supported by the current schema. Actual intra-minute following is a different runtime contract and should not be simulated by repeatedly regenerating the already-persisted full target candle.

## 1. External ticker archive: useful evidence, with important limits

- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/workers/market_feed.rs:129-184` maps provider to `bitget`, `htx`, or `coinbase`, normalizes the symbol, requires positive price/generation, and inserts `market_price_ticks` before normal ingestion. The external event key hashes source, symbol, provider observation timestamp, and normalized price; `source_version` is that event key, **not the feed configuration version**. Duplicate event keys are no-op updates. Different prices at the same provider timestamp can produce separate rows.
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/workers/market_feed.rs:190-204` archives before calling the Redis/Mongo ingestion adapter. Thus an archived row proves that the collector stored an observation, not that Redis accepted it, that publication completed, or that it passed a reference-specific freshness policy. The archive method itself does not reject future observation times.
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/migrations/0114_event_time_price_snapshots.sql:1-20` supplies immutable event identity, decimal price, source, `observed_at`, `generation`, `source_version`, and a separate database `ingested_at`. The `(symbol, observed_at, source, id)` index supports bounded event-time reads. There is no exact pair FK, feed-config FK, transport kind, timestamp-origin flag, or source-finality field.
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/workers/market_feed.rs:375-382` initializes the generation counter inside each supervisor process. Do not compare archive generation directly with an Admin config version or treat it as a globally unique cross-restart provider epoch.

Timestamp quality is not uniform:

- Bitget WS ticker requires item/frame `ts`: `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/market/infrastructure/adapters/provider.rs:414-435`.
- HTX WS ticker requires frame/item `ts`: `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/market/infrastructure/adapters/provider.rs:518-542`.
- Coinbase REST ticker wraps the response with local `Utc::now()`: `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/market/infrastructure/adapters/provider.rs:816-835`. Other REST wrappers can substitute local time when upstream `ts` is absent: the same file, `1051-1058`.
- Consequently, historical rows currently cannot distinguish exchange event time from local response-observation time. A new reader can enforce the archive's recorded-time age, but cannot retroactively prove actual exchange tick freshness or WS provenance. Strict provider-event freshness requires additive timestamp-origin/transport evidence at ingestion, and legacy rows must not silently acquire that guarantee.

## 2. Mongo 1m data is not a confirmed-close ledger

- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/market/domain.rs:432-444` contains provider, symbol, interval, open time, OHLCV and observation time only. There is no `closed`, finality token, or exchange close timestamp. Constructor validation does not establish time alignment or OHLC validity (`448-474`).
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/market/infrastructure/adapters/ingestion.rs:811-844,893-926` stores decimal-string OHLCV, `source`, and `updated_at=observed_at`; updates replace the whole shape. The unique identity is interval/open time, not provider (`860-867`), so switching providers can replace a candle's values and provenance in-place.
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/market/infrastructure/adapters/provider.rs:454-491,560-588` parses Bitget/HTX OHLCV without finality. Missing frame observation time becomes candle open time. Coinbase WS defaults can represent 5m rather than 1m (`695-713`). REST candle observation times may be request/wrapping time (`752-770,844-867`).
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/market/infrastructure/adapters/ingestion.rs:374-386` gates even historical external candle insertion through the Redis latest-slot CAS. A late final update for an older slot may be rejected after a newer slot has arrived, leaving the older Mongo candle at its last partial value. The presence of the next candle is therefore not proof that the preceding one reached final OHLCV.

For a lagged **confirmed-candle** contract, require new explicit provider finality or an independently persisted/frozen final-candle acceptance event. At minimum, an existing candle candidate must have exact aligned expected minute, external pinned source, valid positive OHLCV, and a credible post-end observation. Those conditions are conservative screening, not a substitute for the missing provider-finality evidence. Do not silently select an older surviving candle when the expected minute is missing.

## 3. Exact reference identity, coverage and freshness

- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/migrations/0004_market_pairs_strategy.sql:1-14` defines exact pair IDs and base/quote asset FKs. `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/market/infrastructure/persistence.rs:35-56` shows the reusable join to asset symbols and pair status/type/precision.
- Resolve a configured `reference_pair_id`, require active pair and active base/quote assets, `market_type=external`, expected base BTC and an explicitly selected quote. Compare its normalized stored symbol with the expected base/quote mapping; BTC-USD, BTC-USDT, and BTC-USDC are distinct references. Do not choose by substring or borrow a similarly named asset.
- Existing public lookups intentionally tolerate normalized-symbol ambiguity: `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/market/infrastructure/persistence.rs:141-161` uses `LIMIT 1`, while `169-195` only checks counts. A follower reader should instead reject multiple pair records sharing its normalized archive symbol, since archived ticks have no pair FK.
- External feed coverage is configured by symbol/provider arrays, not by exact pair ID: `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/admin/infrastructure/market_feed.rs:18-30,327-350`. Enabled configuration is not evidence that a fresh BTC frame exists. Runtime status exposes aggregate configuration/readiness but no per-symbol last-event timestamp: `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/workers/market_feed.rs:303-313`.
- Redis is not the preferred replayable reference: `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/market/infrastructure/persistence.rs:217-229` only deserializes the latest ticker; it does not validate positive price or freshness. Its 24h percentage is not the return of the preceding minute.
- Reuse the **pattern**, not the financial module dependency, from `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/convert/service.rs:330-375`: exact identity, positive decimal price, supported source/nonempty identity, reject future timestamps, and bound age using a supplied database clock. Follow-reference policy must exclude both `strategy` and `default`, which this older convert validator does not do.

## 4. Two contracts and the smallest integration seam

### A. One-minute-lagged evidence (smallest scope)

For target minute starting at `T`, compute an explicit preceding-interval reference return, for example `R = P_end / P_start - 1`, using one fixed external provider:

1. Read one database decision time and fix the target UTC minute. Choose `P_end` from a bounded interval immediately before `T`, and `P_start` from a bounded interval immediately before `T-60s`. Use half-open boundaries; do not pick future observations. Require both endpoint freshness limits, distinct ordered observations and a sensible time separation.
2. Pin the reference pair/provider. Do not splice Bitget start price with Coinbase end price, or switch quote currency to fill an empty window. Reject ambiguous same-provider equal-time conflicting prices rather than choosing an arbitrary candidate.
3. Freeze selected IDs/event keys, raw prices, recorded observation/ingestion times, exact reference identity, interval, calculated return and coefficient in the current target-minute state. Do not reselect on same-minute retry: delayed historical arrivals can otherwise change the deterministic result. An `ingested_at` decision cutoff also prevents historical-preview look-ahead, but persisted selected evidence remains the retry authority.
4. Feed a validated, bounded decimal return into the pure target candle generator. Preserve its target initial-price/precision/price-limit contract; do not copy BTC's absolute price. Return scaling, noise and missing-reference behavior must be explicit settings/decisions.

The seam is the **new-minute** branch of `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/workers/synthetic_market/default_market.rs:159-206`, before the immutable minute is serialized and persisted (`207-239`). Existing current-minute state reuse must not fetch new reference evidence. Put SQL evidence loading in market infrastructure and independent validation/return arithmetic in a pure market module. No new runtime HTTP client is needed.

This can truthfully be called a **previous-minute sampled return mapping** using current storage. It is not true live following or provider-confirmed final-candle replication. If confirmed Mongo candles are required instead, the finality changes above are additional necessary scope.

### B. Actual live return mapping (larger scope)

Each target event would consume a newly accepted reference event and map the reference return since a persisted anchor (or since the last consumed reference event), with bounded coefficient/noise and target precision. Repeated reference IDs must not apply a return twice. Current OHLCV must be accumulated from emitted target events; the already-published open/high/low/volume must not be rewritten from a newly chosen full-minute curve. Persist reference cursor, target current OHLCV and mapping anchors for restarts, and freeze each event's evidence before publication.

The present generator persists a complete deterministic candle before its first event and then derives forming values from that fixed candle (`default_market.rs:196-247`). Replacing only its reference return every second violates that contract. True live mapping can still consume local MySQL evidence without direct provider calls, but needs this distinct event-state design, source-delay semantics, cursor/replay handling and additional tests.

## 5. Required guards and verification matrix

- **No loops:** reference pair differs from target; exact metadata must remain external; accepted source allowlist excludes strategy/default. Recheck after metadata changes. Restricting this first feature to external references avoids dependency-graph recursion entirely. Supporting generated references later requires explicit cycle detection and ordered propagation.
- **No fabricated input:** missing/stale/future/invalid/ambiguous evidence produces a visible reference-unavailable decision. Do not silently claim successful BTC following while using zero return, a different symbol/provider, target's own generated price, or arbitrary stale data. Independent baseline continuation can be a separately named approved policy.
- **No look-ahead or repeated application:** UTC boundaries, database clock, ingestion cutoff, immutable selected evidence and exact decimal arithmetic. Never use provider `price_change_percent_24h` as a one-minute incremental percentage.
- **Tests:** pure endpoint selection/validation with fixed clocks, boundary equality, missing adjacent minute, stale and future rows, negative/zero price, extreme decimals, duplicate conflicting timestamps, provider/quote mismatches, self/generated references, late arrivals, and idempotent same-minute evidence reuse. Real isolated MySQL tests should cover source filtering, metadata change, normalized-symbol collisions, stable tie handling and `ingested_at` cutoffs. Mongo-based mode additionally needs interrupted partial candles, late final frames rejected by latest-slot CAS, provider replacement, and absent finality.
- Existing local examples: `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/tests/market_ingestion.rs:204-283` verifies Mongo source and decimal writes; `664-727` verifies stale-candle rejection. `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/tests/market_feed_worker.rs:479-752` covers REST configuration/request-time behavior. These were inspected, not executed in this research slice.

## Verification performed

Read repository instructions, recent progress and applicable backend contracts; traced all above source contracts without accessing data services. Only this research file was written. Source-reference paths/line bounds and this file's whitespace were checked locally after writing. No claim is made about current production BTC availability, provider exchange latency, or completed implementation.
