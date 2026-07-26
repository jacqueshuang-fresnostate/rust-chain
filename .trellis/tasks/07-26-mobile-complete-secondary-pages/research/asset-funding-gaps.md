# Research: Mobile asset and funding workflow gaps

- Query: Compare the production mobile asset/funding workflows with `mobile/sites-prototype/app/page.tsx`, covering asset detail, deposit asset/network/detail, withdraw asset/form, transfer, wallet ledger, quick recharge, and account breakdown; recommend prototype pages, realistic states, and acceptance checks.
- Scope: internal
- Date: 2026-07-26

## Findings

### Executive summary

- The prototype has only six root `View` values and local `activeView` state; it has no secondary-page or URL route model (`mobile/sites-prototype/app/page.tsx:54`, `mobile/sites-prototype/app/page.tsx:1214-1245`).
- Every asset operation currently terminates in a toast. Deposit, withdrawal, transfer, quick recharge, ledger, and the three account rows do not open a usable workflow (`mobile/sites-prototype/app/page.tsx:976-981`, `mobile/sites-prototype/app/page.tsx:1042-1066`).
- The production mobile client already has route-backed deposit, withdrawal, ledger, and quick-recharge flows. Transfer is a stateful bottom sheet on the assets page. Asset detail and account breakdown do not have production routes (`mobile/src/router/index.ts:63-70`, `mobile/src/views/AssetsView.vue:143-168`).
- Prototype completion should therefore add ten secondary surfaces: asset detail; deposit asset, network, and detail; withdrawal asset and form; transfer; ledger; quick recharge; and account breakdown. Reuse the production route hierarchy where one exists, and make transfer route-addressable even if it remains visually presented as a sheet.

### Route / interaction / gap matrix

| Workflow | Production mobile behavior | Prototype today | Gap and recommended prototype surface | Realistic states | Acceptance check |
|---|---|---|---|---|---|
| Asset detail | No dedicated route. Assets are merged by symbol across funding and margin wallets, with available/frozen/locked totals and a per-symbol funding/contract summary (`mobile/src/views/AssetsView.vue:33-48`, `mobile/src/views/AssetsView.vue:152-153`). | Allocation is static and no individual asset row exists. | Add `/assets/:asset` (`asset-detail`). Show total, fiat estimate, funding versus contract allocation, available/frozen/locked, and Deposit/Withdraw/Transfer/Ledger actions. | Loading; positive balance; zero balance; missing market quote; stale/hidden fiat estimate; API error. | Row opens the selected symbol; wallet component totals reconcile; no quote renders an explicit unavailable estimate rather than `$0`; actions preserve the selected asset. |
| Deposit asset | `/assets/deposit`; authenticated list from `GET /wallet/deposit-assets`, searchable by symbol, with load-error and empty states (`mobile/src/router/index.ts:64`, `mobile/src/views/DepositAssetView.vue:17-50`, `mobile/src/api/wallet.ts:86-96`). | Deposit button only raises “充值网络选择已打开”. | Add `/assets/deposit` (`deposit-asset`) before network selection. | Login required; loading; populated list; search result; no search result; no supported assets; request error/retry. | Deposit opens asset picker, not a toast; search is case-insensitive; selecting USDT advances with `asset=USDT`; empty and request-error copy are distinct. |
| Deposit network | `/assets/deposit/:asset/networks`; loads the asset minimum and configured networks, warns that source and destination networks must match, then advances with the exact network key (`mobile/src/router/index.ts:65`, `mobile/src/views/DepositNetworkView.vue:24-56`). | No surface. | Add `/assets/deposit/:asset/networks` (`deposit-network`). | Loading; multiple networks; one network; no configured network; validation/API error. Example fixture: TRON ≈1 min, Ethereum/ERC20 ≈7 min, Arbitrum ≈19 min, but availability stays configuration-driven (`mobile/src/api/wallet.ts:113-123`, `mobile/src/api/wallet.ts:234-239`). | Selected asset remains visible; each row shows display name, ETA, and minimum; selecting a row preserves the backend network key; no network shows a recoverable empty state. |
| Deposit detail | `/assets/deposit/:asset/:network`; posts `{asset_symbol, network}`, generates a QR code, supports clipboard fallback, displays address, optional Memo, account, minimum, ETA, and wrong-asset/network warnings (`mobile/src/router/index.ts:66`, `mobile/src/views/DepositDetailView.vue:23-70`, `mobile/src/api/wallet.ts:125-137`). | No surface. | Add `/assets/deposit/:asset/:network` (`deposit-detail`). | Generating address; ready; Memo present/absent; copy success; allocation/validation failure; retry. | QR encodes the full address; copy feedback is visible and resets; full address remains obtainable even if visually shortened; Memo is shown only when present; asset/network warnings are always visible. |
| Withdraw asset | `/assets/withdraw`; authenticated list from `GET /wallet/withdraw-assets`, searchable by symbol/name, showing the visible fee (`mobile/src/router/index.ts:67`, `mobile/src/views/WithdrawAssetView.vue:22-65`, `mobile/src/api/wallet.ts:98-111`). | Withdrawal button only raises a toast. | Add `/assets/withdraw` (`withdraw-asset`). | Login required; loading; supported list; filtered list; no supported asset; request error. | Withdrawal opens picker; disabled assets are absent; symbol/name search works; choosing an asset advances to its form; fee formatting respects asset precision. |
| Withdraw form | `/assets/withdraw/:asset`; concurrently loads withdrawal metadata, wallet balance, and networks; captures network, address, amount, optional fund password/TOTP; previews fee/arrival; submits with a generated idempotency key and refreshes balance after success (`mobile/src/views/WithdrawView.vue:36-94`, `mobile/src/views/WithdrawView.vue:99-122`, `mobile/src/api/wallet.ts:151-168`). | No surface. | Add `/assets/withdraw/:asset` (`withdraw-form`). Use a review/confirmation state before simulated success. | Loading; unavailable asset; empty address; invalid/over-balance amount; no network; security required/optional; submitting; accepted; API/security failure. | “All” leaves room for the displayed fee; invalid or over-balance requests cannot submit; double tap cannot create two submissions; success clears sensitive inputs and exposes Ledger; amount/fee/arrival use precision-safe fixtures. |
| Transfer | Production uses a bottom sheet on `/assets`, not a route. It switches funding↔contract source, changes the eligible asset list, validates positive/available amount, posts to `/margin/transfers`, refreshes both account sets, and shows success/error feedback (`mobile/src/views/AssetsView.vue:46-48`, `mobile/src/views/AssetsView.vue:74-130`, `mobile/src/views/AssetsView.vue:157-168`, `mobile/src/api/wallet.ts:225-231`). | Transfer button only raises a toast. | Add route-addressable `/assets/transfer` (`transfer`), which may render as a full page on narrow screens or a route-backed sheet. | Funding→contract; contract→funding; no eligible assets; zero balance; invalid/over-balance amount; submitting; success; conflict/API failure. | Direction swap changes source balance and eligible assets; source and destination cannot match; over-balance is blocked; success updates both displayed account balances; close/back returns to Assets without leaving stale success text. |
| Wallet ledger | `/assets/ledger`; requests 30 rows with offset and optional change type, supports All/Deposit/Trade/Contract filters, sorts newest first, displays signed amount and balance-after, preserves unknown change types, and paginates (`mobile/src/views/WalletLedgerView.vue:13-80`, `mobile/src/views/WalletLedgerView.vue:83-95`, `mobile/src/api/wallet.ts:170-183`). | “流水” only raises a toast. | Add `/assets/ledger` (`wallet-ledger`) with optional `asset` and `scope` filter context from detail pages. | Loading; populated mixed entries; each filter; empty filter; request error; loading-more; exhausted; unknown change type. | Filter resets offset; load-more appends without duplicates; rows remain newest-first; positive/negative signs and balance-after are visible; unknown backend type remains readable; empty and error states differ. |
| Quick recharge | `/assets/quick-recharge`; loads config and recent orders, validates configured min/max, sends platform return target, creates an order, optionally opens `paymentUrl`, shows disabled config, and lists recent order status (`mobile/src/views/QuickRechargeView.vue:23-77`, `mobile/src/views/QuickRechargeView.vue:80-96`, `mobile/src/api/wallet.ts:185-223`). | Quick-recharge button only raises a toast. | Add `/assets/quick-recharge` (`quick-recharge`). Simulate the external-payment handoff rather than navigating away from the prototype. | Loading; enabled; disabled; min/max boundary; invalid amount; creating; payment URL ready; payment URL pending; provider error; empty history; `created`/`pending`/`paid`/`failed`/`expired` orders (`src/modules/quick_recharge/service.rs:350-357`). | Min and max are inclusive; disabled config retains the on-chain deposit fallback; submit is single-flight; payment-ready and payment-preparing are distinct; status badges cover all five backend states; recent order is inserted once. |
| Account breakdown | No production route. The production asset list only embeds funding and contract availability per symbol; the prototype has static Spot/Contract/Earn account rows whose clicks only raise toasts (`mobile/src/views/AssetsView.vue:152`, `mobile/sites-prototype/app/page.tsx:1052-1066`). | Static account totals and daily changes, no detail. | Add `/assets/accounts/:scope` (`account-breakdown`) for `funding`, `contract`, and `earn`. Funding and contract should use real wallet concepts; Earn is a clearly labeled illustrative prototype state until a shared account-summary contract exists. | Loading; positive holdings; zero holdings; frozen/locked funds; positions attached to contract account; empty Earn products; API error. | Account total equals visible holdings; each holding can reach asset detail; frozen/locked are not counted as available; contract view distinguishes wallet balance from position PnL; unsupported account data is labeled unavailable, not fabricated. |

### Recommended prototype hierarchy

```text
assets
├── asset-detail/:asset
│   ├── deposit/:asset/networks → deposit/:asset/:network
│   ├── withdraw/:asset
│   ├── transfer?asset=:asset
│   └── ledger?asset=:asset
├── deposit → deposit/:asset/networks → deposit/:asset/:network
├── withdraw → withdraw/:asset
├── transfer
├── ledger
├── quick-recharge
└── accounts/:scope
```

- Implement a prototype navigation stack (`currentPage`, params, back stack) or actual Next routes. Do not extend the current toast callback as the navigation mechanism.
- Keep root bottom navigation on the six primary views. Secondary pages need a sticky back header and no bottom navigation, matching the production navigation contract (`.trellis/spec/mobile/navigation-and-localization.md:47-53`).
- Use route parameters for selected asset/network/scope so direct-open fixtures and back behavior can be tested.

### Shared realistic state model

Every secondary surface should expose the states that apply to it:

1. `login-required` — login CTA preserves the intended internal destination.
2. `loading` — stable skeleton or status without showing a false empty state.
3. `ready` — realistic populated fixture data.
4. `empty` — successful request with zero rows/options.
5. `error` — recoverable request/provider error with retry.
6. `invalid` — field-level validation for forms.
7. `submitting` — primary action disabled and input state retained.
8. `success` — durable result/next action, not only a transient toast.

Recommended fixture checks:

- Asset totals include `available + frozen + locked`, while “available” actions use only `available`; this matches the account response contract (`src/modules/wallet/presentation.rs:177-186`).
- Ledger fixtures include funding credit/debit, spot freeze/unfreeze/fill, margin open/close/liquidation, quick recharge, and one unknown `change_type`; backend rows also expose balance snapshots and reference fields (`src/modules/wallet/presentation.rs:188-230`).
- Deposit fixtures include one asset with several configured networks, one with one network, and one with no network. Network choice must remain an exact backend key because address allocation validates active network configuration and asset eligibility (`.trellis/spec/backend/deposit-addresses.md:12-32`).
- Withdrawal fixtures include fixed fee and tier-boundary amounts. The backend is authoritative for tiered fees even though the current mobile preview reads `withdraw_fee` from the asset list (`.trellis/spec/backend/wallet-amount-precision.md:43-68`).
- Quick recharge fixtures include enabled/disabled configuration, absent/present payment URL, and all five accepted statuses. The backend transitions a newly inserted order from `created` to `pending`, `failed`, or callback-driven `paid` (`src/modules/quick_recharge/application.rs:130-186`, `src/modules/quick_recharge/infrastructure.rs:368-440`, `src/modules/quick_recharge/infrastructure.rs:473-486`).

### Cross-page acceptance checks

- All seven existing asset/account toast targets are replaced by navigable secondary states; toast remains feedback only.
- Direct-open secondary pages have deterministic back fallbacks, and normal forward/back navigation preserves selected asset, network, filter, and account scope.
- Login redirect accepts only internal paths and returns to the intended secondary page after authentication.
- At 390px width: no horizontal overflow, sticky header remains visible, bottom CTA is reachable above safe area/keyboard, sheets scroll internally, and tap targets remain usable.
- Secondary pages use Lucide icons, support reduced motion, and keep fixed user-facing copy ready for localization.
- Amount fixtures test zero, minimum, exact maximum, above maximum, insufficient available balance, and more fractional digits than the asset allows.
- Financial mutations show single-flight submitting state and a persistent result state. Prototype code must never imply a real deposit, withdrawal, transfer, or purchase occurred.
- Add interaction tests for opening each surface, back behavior, form boundaries, and success/error transitions. The current prototype tests only assert source/SSR presence and explicitly codify toast-only asset actions (`mobile/sites-prototype/tests/rendered-html.test.mjs:26-61`, `mobile/sites-prototype/tests/rendered-html.test.mjs:81-101`).

## Files found

- `mobile/sites-prototype/app/page.tsx` — standalone React prototype; six root views and toast-only asset/account actions.
- `mobile/sites-prototype/tests/rendered-html.test.mjs` — SSR/source-regex tests; currently protects the toast-only funding entry pattern.
- `mobile/src/router/index.ts` — production mobile route hierarchy and back fallbacks for all existing funding pages.
- `mobile/src/views/AssetsView.vue` — wallet aggregation, account summaries, funding actions, and transfer sheet.
- `mobile/src/views/DepositAssetView.vue` — authenticated/searchable deposit asset picker.
- `mobile/src/views/DepositNetworkView.vue` — asset-aware network picker and minimum/ETA display.
- `mobile/src/views/DepositDetailView.vue` — address allocation, QR, copy, Memo, and deposit warnings.
- `mobile/src/views/WithdrawAssetView.vue` — searchable withdrawal asset picker with fee.
- `mobile/src/views/WithdrawView.vue` — withdrawal form, security inputs, validation, submit, and post-success refresh.
- `mobile/src/views/WalletLedgerView.vue` — ledger filters, change-type mapping, signed rows, and pagination.
- `mobile/src/views/QuickRechargeView.vue` — config, amount validation, provider handoff, result, and recent orders.
- `mobile/src/api/wallet.ts` — production mobile wallet/deposit/withdraw/ledger/quick-recharge/transfer adapters.
- `mobile/src/api/trading.ts` — margin wallet adapter used for contract-account balances.
- `src/modules/wallet/routes.rs` and `src/modules/wallet/presentation.rs` — authenticated wallet endpoints and account/ledger/withdrawal transport contracts.
- `src/modules/margin/routes.rs` and `src/modules/margin/presentation.rs` — margin wallet and transfer endpoints, including optional transfer idempotency key.
- `src/modules/quick_recharge/routes.rs`, `application.rs`, `service.rs`, and `infrastructure.rs` — quick-recharge routes, amount/order flow, accepted statuses, and provider transitions.

## Code patterns

- Production drill-down routes use increasing `meta.depth`, hide bottom navigation, and provide `backFallback` (`mobile/src/router/index.ts:63-70`).
- Production funding pages guard unauthenticated users with a reusable state that preserves `route.fullPath` as the login redirect (`mobile/src/components/LoginRequiredState.vue:9-23`).
- Back actions go through `goBackOr` rather than raw history, allowing direct-open fallback (`mobile/src/components/PageHeader.vue:15-31`).
- Deposit network availability comes from backend configuration; address assignment posts the selected asset/network pair (`mobile/src/api/wallet.ts:113-137`).
- Wallet totals and transfer availability are intentionally different: totals include available/frozen/locked, while transfer validates against available only (`mobile/src/views/AssetsView.vue:40-48`, `mobile/src/views/AssetsView.vue:93-124`).
- Unknown ledger change types remain visible instead of being silently mislabeled (`mobile/src/views/WalletLedgerView.vue:59-74`).
- Quick recharge uses platform-specific return targets and can return an order before a payment URL is available (`mobile/src/views/QuickRechargeView.vue:23-30`, `mobile/src/views/QuickRechargeView.vue:72-75`, `mobile/src/views/QuickRechargeView.vue:92`).

## External references

- None. This research is intentionally based on the repository's production mobile client, backend contracts, and project specs; no third-party design or API documentation was required.

## Related specs

- `.trellis/spec/mobile/navigation-and-localization.md` — secondary route depth, hidden bottom navigation, safe back fallback, login redirect, and localization requirements.
- `.trellis/spec/backend/deposit-addresses.md` — configured deposit network/address-group eligibility and validation.
- `.trellis/spec/backend/wallet-amount-precision.md` — wallet amount precision, ledger snapshot consistency, and tiered withdrawal fee authority.
- `.trellis/spec/backend/margin-trading-actions.md` — atomic funding↔margin transfers, validation, idempotency, and wallet/ledger consistency.
- `.trellis/spec/backend/error-handling.md` — safe quick-recharge provider failure responses.
- `.trellis/spec/guides/cross-layer-thinking-guide.md` — API/UI boundary and round-trip verification checklist.

## Caveats / Not Found

- `python3 ./.trellis/scripts/task.py current --source` returned no active task, although the user supplied this exact task directory and `task.json` exists with `status: planning`. The research was written to the explicit path; no task pointer was changed.
- No production mobile route/component was found for asset detail or account breakdown. Recommendations for those two pages are inferred from wallet/margin response fields and the current prototype account concepts.
- The production withdrawal UI previews the flat `withdraw_fee`, while the backend may calculate a tiered fee from the submitted amount. The prototype should show tier-aware states and label the preview as final only after a server-authoritative review step.
- Deposit ETA values in the production mobile adapter are client heuristics based on the network string, not a backend SLA (`mobile/src/api/wallet.ts:234-239`).
- The production mobile transfer adapter does not send an idempotency key, although the backend request accepts one and the margin spec defines replay behavior (`mobile/src/api/wallet.ts:225-231`, `src/modules/margin/presentation.rs:70-78`, `.trellis/spec/backend/margin-trading-actions.md:23-27`).
- No dedicated mobile tests were found for these funding screens. Existing mobile tests cover navigation helpers, while prototype tests are SSR/source assertions rather than client interaction tests.
- `docs/superpowers/PROGRESS.md` was read as required but not updated because this research agent is explicitly restricted to writes inside the task's `research/` directory.
