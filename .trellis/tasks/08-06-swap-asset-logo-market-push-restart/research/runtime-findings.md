# Runtime findings

## Swap asset images

- `mobile/src/components/AssetMark.vue` accepts `src?: string` and already resets its fallback state when the URL changes.
- `fetchWalletAccounts()` maps backend `logo_url` to `WalletAccount.logoUrl`.
- `SwapView` renders `AssetMark` in the pay card, receive card, and picker rows without passing `src`.
- Ego runtime check at `http://127.0.0.1:4178/#/swap` showed two marks with text fallbacks `U` and `B`, both with `image: null`.

## Restarted market feed

- Remote WebSocket `wss://hipoex.cllbmz.kdns.fr/api/v1/ws/public` opened successfully and acknowledged `ticker/BTCUSDT` subscription.
- The same connection received no ticker frame during a 12-second observation window.
- Two public REST ticker reads seven seconds apart returned the same payload and the same `observed_at=1785928408307`.
- Public `/api/v1/markets` currently returns the active external market `BTC-USDT`.
- `src/main.rs` loads an enabled database config once and otherwise builds from `Settings.market_feed_*`; empty symbols disable the worker.
- `docker-compose.1panel.yml`, `docker-compose.1panel.example.yml`, and `docker-compose.example.yml` configure provider URLs but omit all three `MARKET_FEED_*` runtime selection variables.
- Therefore a restart with no enabled database row (or a startup read failure) accepts client subscriptions but has no ingestion worker publishing into the new in-memory broadcast hub.

## Scope choice

Add explicit environment fallback values to the deployment surfaces actually used to start the API. Do not change the public protocol or auto-infer arbitrary database markets in this slice.
