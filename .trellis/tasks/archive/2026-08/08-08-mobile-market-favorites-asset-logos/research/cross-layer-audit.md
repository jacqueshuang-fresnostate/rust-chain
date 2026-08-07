# Market favorites and backend Logo audit

## Existing market Logo path

- `src/modules/market/infrastructure.rs::list_active_markets` reads `trading_pairs.logo_url` and exposes it through `MarketResponse`.
- `mobile/src/core/marketMapper.ts` maps that field to `MarketTicker.iconUrl`.
- `HomeView.vue`, `MarketsView.vue`, `TradeView.vue`, and `MarketDetailView.vue` already pass `ticker.iconUrl` to `AssetMark`.
- The deployed `GET https://hipoex.cllbmz.kdns.fr/api/v1/markets` returned a BTC-USDT `logo_url` on 2026-08-08, but GET/HEAD against that exact external URL returned HTTP 500. The current visible letter is therefore the component's intended broken-image fallback, not a hard-coded coin icon.
- The public market query does not select `base.logo_url` or `quote.logo_url`, so there is no backend-owned fallback image after the pair image fails.

## Existing wallet Logo path

- `src/modules/wallet/infrastructure.rs` joins `assets.logo_url` for spot wallets, and `mobile/src/api/wallet.ts` maps it to `WalletAccount.logoUrl`.
- `mobile/src/views/AssetsView.vue` already passes the merged row `logoUrl` into `AssetMark`.
- `src/modules/margin/presentation.rs::MarginWalletAccountResponse` and `src/modules/margin/infrastructure.rs::list_margin_wallet_accounts` omit the asset Logo, and `mobile/src/api/trading.ts::fetchMarginWallets` consequently cannot populate `logoUrl`.

## Existing favorites behavior

- No backend migration, repository, route, or API contains a market favorites/watchlist implementation.
- `MarketsView.vue` uses a component-only `Set` and loses it on navigation/reload.
- `TradeView.vue` and `MarketDetailView.vue` duplicate a `hippo-mobile-market-favorites` localStorage implementation.
- `HomeView.vue` intentionally returns `[]` for the favorites tab.
- PC has an independent `pc.market.favoriteSymbols` localStorage path and is outside the requested mobile scope.

## Minimal implementation

1. Add migration `0100_user_market_favorites.sql` with FK cascades and a unique `(user_id, trading_pair_id)` key.
2. Extend the market bounded context with authenticated `GET/PUT/DELETE /user/market-favorites` handlers. Normalize symbols with existing `ValidatedMarketSymbol`; lookup only active trading pairs; use `INSERT ... ON DUPLICATE KEY UPDATE` and unconditional delete for idempotency.
3. Extend public `MarketResponse` and favorite response rows with pair/base/quote Logo fields from the existing database columns.
4. Add `assets.logo_url` to margin wallet rows and the mobile mapper.
5. Add a shared mobile API and Pinia store; initialize/reset it from the root session lifecycle; replace the four divergent page implementations.
6. Extend `AssetMark` with one backend-owned fallback source so pair image failure can try the base asset image before initials.

## Test locations

- Rust: `tests/market_routes.rs` for public Logo fields and authenticated favorite CRUD/isolation; `tests/margin_routes.rs` for wallet Logo response; a migration source contract test for table/FK/unique-key shape.
- Mobile: `mobile/tests/market-mapper.test.ts`, a new favorites integration source/adapter contract test, and existing root/market-detail/spot layout tests.
