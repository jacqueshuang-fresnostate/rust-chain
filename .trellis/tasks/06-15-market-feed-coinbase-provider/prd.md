# 行情订阅新增 Coinbase provider

## Goal

Add Coinbase Advanced Trade as a market feed provider so admins can select `coinbase` in the market subscription configuration, and the backend can build Coinbase WebSocket subscriptions plus REST fallback frames using the same market ingestion pipeline as Bitget and HTX.

## Requirements

* Backend provider validation accepts `coinbase` while keeping the existing single-enabled-provider admin rule.
* Runtime settings include Coinbase REST and WebSocket base URLs with environment parsing support.
* Market feed worker can generate Coinbase WebSocket subscriptions for ticker, trades, depth, candles, and REST fallback URLs for ticker/candles.
* Market adapters can parse Coinbase ticker, depth, candle, and trade payloads into the existing internal snapshots.
* Admin market feed configuration page exposes Coinbase as a provider option and keeps one provider selected at a time.

## Acceptance Criteria

* [ ] `MarketFeedProvider::from_code("coinbase")` succeeds and unknown providers still fail.
* [ ] Provider config tests include Coinbase URL and subscription payload coverage.
* [ ] Coinbase sample ticker/candle/trade/depth payloads parse into existing cache snapshot types.
* [ ] Admin page tests show three provider rows and can save `providers: ["coinbase"]`.
* [ ] Focused Rust and frontend tests pass.

## Definition of Done

* Tests added or updated for backend provider/runtime and admin UI behavior.
* Formatting/type checks run for the touched layers where practical.
* Progress is recorded in `docs/superpowers/PROGRESS.md`.

## Technical Approach

Extend the existing provider enum and provider-specific helper methods instead of adding a parallel Coinbase path. Coinbase product IDs will be derived from validated platform symbols by inserting a dash before common quote assets, then converted back to compact symbols for internal snapshots.

## Research References

* [`research/coinbase-advanced-trade.md`](research/coinbase-advanced-trade.md) — Coinbase Advanced Trade REST/WS endpoint, channel, and symbol-format notes.

## Decision (ADR-lite)

**Context**: The existing worker treats each external provider as a thin adapter around one internal market ingestion contract.

**Decision**: Add Coinbase as a first-class `MarketFeedProvider` with provider-local subscription, REST URL, and adapter conversion functions.

**Consequences**: This keeps admin/API contracts unchanged (`providers: string[]`) and preserves the current one-provider UI behavior. Unsupported Coinbase product IDs will fail at the provider response level rather than adding a separate product mapping table in this task.

## Out of Scope

* Coinbase authenticated private endpoints.
* Admin credential fields specific to Coinbase API keys.
* Dynamic product discovery or symbol alias management.

## Technical Notes

* Coinbase Advanced Trade docs identify WebSocket endpoint `wss://advanced-trade-ws.coinbase.com` and public REST market data under `/api/v3/brokerage/market/...`.
* Existing provider files inspected: `src/modules/market/mod.rs`, `src/workers/market_feed.rs`, `src/modules/admin/market_feed_config.rs`, `web/src/admin/actions/MarketFeedConfigPage.tsx`.
* The admin validator currently deduplicates provider aliases and rejects more than one selected provider.
