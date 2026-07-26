# Complete Mobile Prototype Secondary Pages

## Goal

Turn the public HIPPO mobile Sites prototype from a polished root-shell demo
into a complete, navigable product prototype. Every visible entry should open a
purpose-built page or perform a meaningful local action; Toast remains feedback,
not navigation.

## Product Baseline

- Keep six root columns: Home, Markets, Spot, Contract, Assets, Profile.
- Mirror the production mobile route hierarchy and PC/backend product scope.
- Use deterministic mock data and local-only mutations. Never imply that a real
  account, order, transfer, identity check, or payment changed.
- Preserve the OKX-referenced black/white exchange visual direction, Lucide-only
  icons, no emoji, compact controls, and public Sites delivery.

## Navigation Foundation

- Introduce a typed logical route stack with route ID, title, params, and origin.
- Root-column changes replace the root state; secondary pages push onto the
  stack; Back pops or returns to a deterministic fallback.
- Hide the global root header and bottom navigation on secondary/auth pages.
- Every secondary page uses a compact sticky header with Back and optional
  actions.
- Preserve selected market, asset, network, product, order tab, and originating
  Spot/Contract column across drill-down and Back.
- Do not retain the generic ProductSheet as the terminal destination.

## Required Secondary Surfaces

### Market, Content, And Trade

- `market-detail`: quote, chart timeframe, depth, recent trades, favorite/share,
  Spot and Contract handoffs.
- `market-picker`: searchable pair picker that returns to the originating
  Spot/Contract column.
- `news-list` and `news-detail`: announcements, categories, publish time, body,
  refresh and empty/error demo states.
- `orders`: Spot open orders, Contract positions, History tabs with simulated
  cancel/close.
- `trade-settings`: pair-aware chart/order settings, Contract margin mode and
  supported leverage with Apply/Cancel.
- `message-center`: Notifications/Announcements tabs, unread/read state,
  mark-all-read, announcement deep links.
- `scanner`: explicit prototype scan state and manual-code fallback.

### Asset And Funding

- `asset-detail`: selected asset total, available/frozen/locked, wallet
  allocation, Deposit/Withdraw/Transfer/Ledger actions.
- `deposit-asset`, `deposit-network`, `deposit-detail`: searchable asset picker,
  network/minimum/ETA choice, address/QR-style payload, copy and warnings.
- `withdraw-asset`, `withdraw-form`: searchable asset picker, network/address,
  amount/fee/arrival review, validation and simulated accepted result.
- `transfer`: Funding/Contract direction, asset, available balance, amount,
  swap direction, validation and simulated result.
- `wallet-ledger`: filters, signed changes, balance-after, reference and
  load-more.
- `quick-recharge`: amount boundaries, provider handoff state and recent orders.
- `account-breakdown`: Funding, Contract and Earn scopes with internally
  reconciled holdings.

### Products

- `buy-crypto`: payment method, asset and amount selection, quote review and
  simulated provider handoff.
- `swap`: pair/reverse, amount validation, quote/expiry, confirm and history.
- `product-hub`: Earn, Loan, New Coins, Prediction and Seconds entries.
- `earn`: products, subscribe validation, holdings and redeem.
- `loan`: public products, collateral fields where applicable, application and
  order status actions.
- `new-coins`, `new-coin-detail`, `new-coin-records`: lifecycle-aware projects,
  subscribe/purchase, four record tabs, fee/release simulation.
- `prediction`: public markets, Yes/No ticket, quote preview/expiry and history.
- `seconds`: pair, direction, cycle, stake, payout and order record.
- `task-center`: verification and first-trade task progress.

### Profile And Authentication

- `profile-edit`: avatar preview state and username validation.
- `kyc`: Personal/Enterprise, country/document type, mock upload slots,
  validation and pending/rejected/approved status presentation.
- `security`: simulated TOTP setup, policy toggle, password/fund-password forms.
- `bindings`: email verification and policy-enabled external identifier binding.
- `referrals`: code copy/bind and invite history.
- `language`: Chinese/English preference with accessible selected state.
- `login`, `two-factor`, `register`, `forgot-password`: local guest/member demo,
  protected-destination return, validation and deterministic completion.

## Entry Wiring

- Global bell and Home message icon open Message Center.
- Home search and Trade symbol selector open the appropriate market browser.
- Home market rows and normal Markets rows open Market Detail.
- Home Buy and Deposit actions open their dedicated flows.
- Home shortcut grid and Product Hub open full product pages.
- Home AI brief opens News Detail; task/security cards open their full pages.
- Trade settings/chart/settings/orders controls open their secondary pages.
- Every Assets action, ledger action, account row, and holding opens a full page.
- Every Profile menu, avatar/settings action, and logout path opens a full page
  or changes the local demo session.

## Interaction Requirements

- Financial forms validate empty, zero, minimum, maximum, precision, and
  insufficient-balance cases where applicable.
- Quote-based flows separate Quote and Confirm states; changing input invalidates
  the quote.
- Submit controls use a single-flight state and create/update exactly one local
  record.
- Pages expose meaningful ready, empty, validation, submitting, and success
  states; selected pages also expose recoverable error/retry demos.
- Protected actions in guest mode open Login and return to the intended route
  after successful local authentication.
- Dialogs/sheets use correct roles and remain scrollable on mobile.

## Acceptance Criteria

- [x] Every visible non-terminal button has a wired page/action; no
      `已打开` navigation Toasts remain.
- [x] All required route IDs are defined in typed source and reachable from at
      least one visible entry or parent page.
- [x] Secondary pages hide root navigation and provide reliable Back behavior.
- [x] Market, asset, product and account drill-down chains preserve parameters.
- [x] Spot and Contract remain independent root columns.
- [x] Financial flows provide validation, review and durable simulated results.
- [x] Profile/auth guest/member behavior is coherent and protected returns work.
- [x] At 390x844 and 1440x900 there is no horizontal overflow, clipped sticky
      CTA, nav occlusion, or incoherent overlap.
- [x] Lucide-only and no-emoji contracts remain true.
- [x] Lint, production build, tests, static route contracts, browser interaction
      checks and clean console pass.
- [x] The exact validated commit is saved and deployed as a public Sites version.

## Out Of Scope

- Calling live backend endpoints or using real authenticated user data.
- Uploading real KYC documents, opening real payment providers, or placing real
  financial orders.
- Modifying production `mobile/src`, PC, or backend behavior.
- Pixel-copying any third-party exchange.

## Technical Notes

- Current root shell: `mobile/sites-prototype/app/page.tsx`.
- Prefer new focused component/data modules over further expanding the existing
  root file.
- Styling remains in the Sites prototype and must follow the existing responsive
  variables and safe-area conventions.
- Research:
  - `research/market-content-gaps.md`
  - `research/asset-funding-gaps.md`
  - `research/product-account-gaps.md`
