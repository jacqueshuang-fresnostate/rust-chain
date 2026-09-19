# Public Market Provenance

## 1. Scope

REST/WS/cache and PC/Mobile presentation of ticker, depth, trades and candles.
Metadata describes existing evidence and never changes price selection, order
triggering, settlement prices or executable inventory.

## 2. Signatures

Additive fields are `source` and nullable `provider`. Tickers retain
`observed_at`; candles additionally expose nullable millisecond `observed_at`
from cache observation or Mongo `updated_at`. Candle open time is not freshness.

Semantic sources: `platform`, `external`, `strategy`, `default`, `generated`,
`unknown`. Providers: `platform`, `bitget`, `htx`, `coinbase`, `strategy`.

## 3. Contracts

- Only real `spot_trades` records are `platform/platform`. External feed
  prints are references, not this platform's executed trading volume.
- Shared Strategy provider proves generated data, not a specific generator.
  Trade subtype requires the validated generator trade-ID/time contract;
  matching same-frame depth can inherit that evidence. Otherwise use
  `generated`, not a guess from current pair configuration.
- Legacy cache/document absence remains `unknown`. Mongo source strings are
  interpreted as recorded producer evidence, never current market settings.
- Preserve provenance with the same owner, timestamp and race handling as
  its values. Late REST cannot relabel a newer WS price. Unknown new evidence
  clears the previous provider rather than retaining its label.
- PC numeric candle compatibility rows retain sidecar provenance that the
  normalized candle model carries to either chart implementation. Mobile
  normalized candles retain per-candle provenance through history merging.
  Chart source summaries show every observed category, including unknown.
- Ticker presentation uses real observation time and marks older-than-60s or
  future-dated evidence stale. Historical closed candles are not labelled as
  stale realtime tickers. Local receipt time is never source observation.
- The Redis Kline sequence key and CAS ordering remain unchanged. Additive
  payload fields may make a pre-upgrade equal-time cache payload different;
  the existing equal-time fence remains in force until a newer observation.

## 4. Error Matrix

| Condition | Presentation |
| --- | --- |
| Missing source and provider | Unknown source |
| Unknown/contradictory source/provider | Unknown, never real trade |
| Legacy provider-only external WS | External reference |
| Shared strategy without subtype evidence | Generated/simulated |
| Missing time | Time unavailable, not fresh by inference |
| Stale or future ticker observation | Stale |

## 5. Cases

- Good: a platform trade and external print with equal IDs coexist because
  identity includes source/provider.
- Base: old candles remain visible with unknown source and nullable time.
- Bad: label all historical candles as strategy because the current pair uses
  a strategy, or treat display depth as a guarantee of inventory.

## 6. Tests

Cover producer-to-cache/REST/WS provenance, legacy absence, contradictory
metadata, mixed chart history, ticker source switching, late REST, numeric
price preservation, timer cleanup and narrow-screen labels. A mocked browser
render test is not a live provider/database integration test.

## 7. Wrong Vs Correct

Wrong: copy the current pair's market type onto every returned candle.

Correct: preserve each candle's persisted source and observation; label
missing evidence unknown without backfill.
