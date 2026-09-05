# Current mobile new-coin implementation audit

## Existing route and business coverage

- `mobile/src/router/index.ts` already owns three typed secondary routes:
  `/products/new-coins`, `/products/new-coins/records`, and
  `/products/new-coins/:symbol`. They hide the Dock, carry depth metadata, and
  have deterministic back fallbacks. No router redesign is required.
- `NewCoinsView.vue` reads public projects and authenticated subscriptions,
  navigates to detail/records, and has truthful loading/error/empty branches.
- `NewCoinDetailView.vue` reads one project, wallets and optional market data;
  it supports subscription and post-listing purchase with exact-decimal helper
  functions, percentage calculation, login redirect, review dialog, focus
  trapping and mutation feedback.
- `NewCoinRecordsView.vue` reads subscriptions, distributions, purchases,
  unlocks and wallets; it supports unlock-fee payment and release with
  authentication, balance checks, scroll lock and keyboard focus containment.

## Visual gaps against the selected frames

- The list is still the older compact text hero, three-state segmented filter,
  113px flat rows, a separate records link and recent-subscriptions section.
  It has none of the selected banner, two-level tabs, five lifecycle filters,
  300px Launchpool card, or Trading Opportunities state.
- Detail is still one compact identity row, four flat facts, a three-step list,
  a 51px entry field and a detached action. It does not implement the selected
  210/112/104/328px section stack.
- Records uses four API-category tabs and 72px table-like rows. The selected
  design requires four status filters and 168px project cards with Logo,
  status rail, two-column metric row and contextual footer.
- Existing scoped styles rely on shared generic tokens and do not declare the
  exact route-specific light/dark palettes from the selected eight frames.

## API/model gaps that block truthful parity

- The backend already serializes `quote_asset_id`, `reserved_supply`,
  `allocated_supply`, and `remaining_supply`, but `mobile/src/api/newCoin.ts`
  does not retain them.
- Public project rows contain only the duplicate project symbol. The related
  `assets` row already owns `name` and nullable `logo_url`, but the public new
  coin query does not join or expose them. Quote-asset symbol/Logo are also
  absent. Joining the existing rows is sufficient; no migration is needed.
- The detail page currently chooses a subscription quote wallet from available
  accounts instead of binding the backend `quote_asset_id`. This can disagree
  with the administrator-configured issuance asset and should be corrected.
- Existing record adapters convert financial values to JavaScript numbers.
  Display should add exact `DecimalText` companions and use them for the new
  cards while preserving numeric compatibility where current mutation checks
  still need it.

## Reusable project patterns

- `useMarketStore()` owns REST cold-start deduplication and a shared ticker
  WebSocket lease. New Coin Zone should call `refresh()` once, acquire a stable
  consumer only while opportunities require live data, and release that exact
  consumer on unmount. It must not directly repeat `fetchMarketTickers()`.
- `AssetMark` already enforces backend-image-first, circular clipping and a
  symbol fallback. Pass project/quote/ticker URLs into it and avoid local image
  decoration.
- `PageHeader`, `useModalDialog`, financial Decimal helpers and the existing
  selected-page global stylesheet provide the required navigation, focus,
  safe-area and theme boundaries.
- The selected banner may be copied from the Pencil-generated asset into a
  tracked production asset; production tests already reject dependencies from
  `mobile/src` to `mobile/pencil`.

## Recommended implementation slices

1. Extend the backend public project read model/DTO and both list/detail SQL
   projections with project asset name/Logo and quote asset symbol/Logo. Add
   configured/null Logo route assertions.
2. Strengthen `mobile/src/api/newCoin.ts` with strict optional text/Logo
   mapping, missing supply/quote fields and exact record decimal companions.
3. Add a pure new-coin presentation helper for lifecycle buckets, progress,
   countdown/next milestone, opportunity filtering and unified chronological
   record rows. Unit-test the helper independently.
4. Rebuild `NewCoinsView.vue`, `NewCoinDetailView.vue` and
   `NewCoinRecordsView.vue` around the exact selected tracks, splitting focused
   cards/components if source-size governance requires it.
5. Add exact route palettes to `pencil-selected-pages.css`, symmetric locale
   keys, focused source contracts and runtime 320/390/448 visual verification.

## Regression risks

- Do not infer a post-listing pair from symbol; only
  `post_listing_pair_id` can navigate or submit a purchase.
- Do not substitute sample APR, project copy, countdowns, IDs, prices or
  quantities to fill Pencil geometry.
- Do not let a late market refresh replace newer WebSocket values or leak a
  ticker lease after route unmount.
- Do not remove unlock fee/release actions while merging the four record APIs.
- Do not shrink pointer targets to the 22–40px visual faces.
