# brainstorm: backend ddd architecture refactor

## Goal

Refactor the backend toward a clearer DDD-style architecture so business rules are no longer buried inside large route handlers, code ownership is easier to understand, Chinese business comments explain non-obvious domain rules, and tests live in dedicated test files instead of inside production modules.

## What I already know

* The user explicitly wants the backend code structure redesigned according to DDD ideas.
* The user wants Chinese comments added where useful.
* The user wants test code separated from business code into standalone files.
* Current backend modules are mostly under `src/modules/*`.
* Several files are very large and mix HTTP routing, request DTOs, SQL, business rules, validation, event publishing, and wallet mutations:
  * `src/modules/admin/routes.rs` has about 9,616 lines.
  * `src/modules/spot/routes.rs` has about 3,883 lines.
  * `src/modules/market/mod.rs` has about 3,409 lines.
  * `src/modules/margin/routes.rs` has about 2,929 lines.
  * `src/modules/prediction.rs` has about 2,508 lines.
* Most integration tests already live under `tests/`, which matches the user's direction.
* Inline tests still exist in production files:
  * `src/modules/loan.rs`
  * `src/modules/spot/routes.rs`
* Existing backend directory-structure spec is still a placeholder and can be updated after the architecture decision is settled.

## Assumptions (temporary)

* This should be an incremental migration, not a risky full-repo rewrite in one pass.
* The first deliverable should establish the target DDD skeleton and migrate one representative module or bounded context.
* Chinese comments should explain business intent and invariants, not repeat obvious Rust syntax.
* Tests should be moved to `tests/` integration files or dedicated domain test files, unless a tiny private helper test must remain temporarily for compiler visibility reasons.

## Open Questions

* User requested doing the backend-wide structure pass in one task instead of selecting only one first module.

## Requirements (evolving)

* Define a consistent DDD-inspired module layout for backend bounded contexts.
* Keep HTTP/API handlers thin: parse request, authorize, call application service, return response.
* Move business rules into domain/application services.
* Add explicit `repository` and `service` layer files for every bounded context.
* Use `repository` for repository interfaces/contracts and `service` for reusable business services.
* Move SQL persistence into infrastructure/repository files.
* Keep DTO/request/response mapping separate from domain entities where practical.
* Add Chinese comments for non-obvious business rules, transaction boundaries, wallet/ledger invariants, and risk-sensitive decisions.
* Move inline tests out of production modules into dedicated files.
* Preserve existing API behavior unless explicitly changed.
* Keep changes incremental and verifiable.

## Acceptance Criteria (evolving)

* [x] A backend DDD directory convention is documented in `.trellis/spec/backend/directory-structure.md`.
* [x] All backend bounded contexts have DDD layer anchor files: `domain`, `repository`, `service`, `application`, `infrastructure`, and `presentation`.
* [x] Inline test bodies are moved out of production modules into standalone files.
* [x] Chinese comments are added to architecture/layer anchors for important layer responsibilities.
* [x] An architecture guard test prevents missing DDD layer files and new inline test bodies.
* [x] Existing backend tests still pass.
* [x] `cargo fmt --manifest-path Cargo.toml --check` and `cargo check --manifest-path Cargo.toml --all-targets` pass.
* [x] Low-risk public contexts (`countries`, `platform`, `news`) have real DDD layer extraction behind stable public route APIs.
* [x] Rule-oriented contexts (`risk`, `security` domain rules) have pure domain extraction without changing public APIs.
* [x] `security` database access lives in `infrastructure`, and security verification orchestration lives in `application` behind stable public re-exports.
* [x] `auth` core repository contract lives in `auth/repository.rs`, MySQL implementation lives in `auth/infrastructure.rs`, and reusable auth token/session orchestration lives in `auth/service.rs` behind stable public re-exports.
* [x] `auth` registration email-code, password reset, and shared email verification workflows are orchestrated in `auth/application.rs`; route handlers no longer contain SQLx queries, password hash writes, or email-code generation logic for those flows.
* [x] `auth` user login 2FA challenge selection, 2FA verification token issuance, and login 2FA reset flows are orchestrated in `auth/application.rs`; route handlers keep only transport DTO mapping for those flows.
* [x] `auth` register/login config lookup and registration invite-policy gating are orchestrated in `auth/application.rs`; route handlers no longer read security policy directly for those flows.
* [x] `auth` admin/agent login, admin registration, refresh-token exchange, and agent-registration rejection are exposed as application use cases; route handlers no longer call `AuthService` directly.
* [x] `user` profile lookup and username update flows are split across presentation/application/infrastructure; route handlers no longer contain the profile SQL or username update transaction.
* [x] `user` 2FA status, setup, confirmation, login-2FA toggle, and reset-status refresh are orchestrated in `user/application.rs`; route handlers no longer contain TOTP setup/verification or login-2FA policy logic.
* [x] `user` email bind-code and bind-confirmation flows are split across presentation/domain/repository/service/application/infrastructure; route handlers no longer contain bind-email SQL, verification-code generation, SMTP delivery, or email-bind audit transaction logic.
* [x] `user` login-password change, fund-password create/change/reset, and verified-email reset-code workflows are orchestrated in `user/application.rs`; route handlers no longer contain those password SQL mutations, token re-issue orchestration, SMTP delivery, or purpose-based email-code verification logic.
* [x] `user` third-party binding status and bind-account workflows are split across presentation/service/application/infrastructure; route handlers no longer contain provider validation, security-policy gating, binding SQL, or audit transaction logic for those flows.
* [x] `user` KYC status and submission workflows are orchestrated in `user/application.rs`; route handlers no longer contain KYC config/submission lookup, submission transaction, or audit orchestration logic.
* [x] `user` referral code, bind-code, and my-invites workflows are split across presentation/repository/service/application/infrastructure; route handlers no longer contain invite-code generation, referral-tree transaction logic, or referral SQL queries.
* [x] `user` avatar upload persistence and 2FA setup user-existence checks are orchestrated outside `user/routes.rs`; user route handlers no longer contain raw SQLx queries.
* [x] `kyc` public request/response DTOs live in `kyc/presentation.rs`, and KYC audit JSON / identity masking helpers live in `kyc/service.rs` behind stable root-module re-exports.
* [x] `kyc` SQL rows, default config bootstrap, QueryBuilder listing, submission lookup, and review/update persistence helpers live in `kyc/infrastructure.rs`; `kyc.rs` now keeps validation and use-case orchestration behind stable public functions.
* [x] `kyc` document type rules, config validation, submission validation, status normalization, and required/optional string normalization live in `kyc/domain.rs`; `presentation` keeps compatibility re-exports for public API payloads.
* [x] `wallet` read-only user queries for accounts, ledger, deposit assets, deposit networks, and withdraw assets use presentation DTOs, application use cases, and infrastructure SQL helpers; route handlers no longer own those QueryBuilder/select queries.
* [x] `wallet` deposit address assignment uses presentation DTOs, application transaction orchestration, and infrastructure SQL helpers for network config lookup, deposit enable checks, address-pool locking, user email lookup, assignment update, and assigned-address loading.
* [x] `wallet` withdrawal creation uses presentation DTOs, application validation/security orchestration, and infrastructure helpers for server-side withdrawal fee calculation and withdrawal request insertion; route handlers no longer own withdrawal SQL or security-verification orchestration.
* [x] `spot` order/trade read-only queries use presentation DTOs, application use cases, and infrastructure QueryBuilder helpers for user order list, admin order list/detail, and admin trade list; route handlers no longer own those read-list SQL filters.
* [x] `margin` user transfer, leverage update, and margin-mode update workflows use presentation DTOs, application transaction orchestration, and infrastructure SQL helpers; route handlers no longer own those wallet transfer or user-setting persistence transactions.
* [x] `margin` close/cancel position workflows use presentation position DTOs, application transaction orchestration, and infrastructure helpers for position locks, batch ID loading, mark-price lookup, wallet refund/settlement ledger writes, and status updates; route handlers only publish events after use-case results.
* [x] `margin` open-position workflow uses presentation DTOs, application transaction orchestration, and infrastructure helpers for idempotency replay, active product locking, cached entry-price lookup, position insertion, wallet collateral debit, and wallet-scope persistence; route handlers only handle auth, pool lookup, use-case call, and private event publication.
* [x] `margin` product/position/wallet read queries and risk snapshot use presentation DTOs, application use cases, and infrastructure SQL/Redis helpers; route handlers no longer own those QueryBuilder filters, position detail SQL, wallet SQL, or risk snapshot assembly.
* [x] `margin` admin product create/update/status workflows use presentation request DTOs, application validation and transaction orchestration, service audit JSON mapping, and infrastructure SQL/audit helpers; route handlers no longer own product validation, SQL writes, product locks, or audit transactions.
* [x] `spot` user/admin order cancellation workflows use presentation validation, application use cases, repository contracts, service state/audit helpers, and infrastructure SQL/transaction helpers; route handlers no longer own cancel transactions, wallet unfreeze, or admin audit writes.
* [x] `spot` create-order execution-price resolution uses application orchestration, infrastructure Redis ticker access, and service-level market/limit/stop-limit rules; route handlers no longer own cached ticker parsing or pre-insert execution-price branching.
* [x] `spot` create-order idempotency replay uses application orchestration, infrastructure idempotency-key lookup, repository records, and service-level request matching; route handlers no longer own pre-insert idempotency SQL or replay comparison rules.

## Definition of Done

* Tests added/updated where behavior boundaries move.
* Lint/typecheck/build checks pass.
* Architecture spec updated.
* Progress recorded in `docs/superpowers/PROGRESS.md`.
* Migration notes clearly state what remains out of scope.

## Added Scope (2026-07-11)

* PC trading K-line rendering must support the official TradingView Lightweight Charts renderer while preserving the platform's own REST and WebSocket market-data contracts.
* Administrators must be able to choose the global PC K-line renderer dynamically from the existing PC display configuration, with validation, persistence, public configuration delivery, and audit logging.
* Existing KLineCharts rendering remains a selectable fallback to support staged rollout and rollback.

## Added Scope (2026-07-11: Mobile Client)

* Add a standalone `mobile/` client instead of forcing desktop views to serve mobile layouts. It must use Vue 3 + Vite for H5 and Tauri v2 for native packaging.
* The client must share the existing `/api/v1` public and authenticated contracts for markets, spot orders, margin positions, wallet accounts, deposit assets/networks, deposit addresses, platform branding, and authentication. It must not introduce duplicate backend business rules or alter current APIs.
* Deliver the mobile information architecture represented by the supplied references, with original Hippo branding and implementation: exchange home, market list, dark market-detail/K-line page, spot/contract trade views, assets, login, coin selection, network selection, and deposit address/QR flow.
* Provide touch-first responsive behavior for iOS, Android, and H5: safe-area insets, dynamic viewport sizing, keyboard-friendly page areas, bottom navigation, minimum touch targets, and constrained large-screen H5 rendering.
* Add Tauri mobile project configuration with a shared Rust `lib.rs` entry point, Vite `TAURI_DEV_HOST` support for physical-device development, and documented Android/iOS scripts. Keep test files in `mobile/tests/`, outside production source directories.

## Added Scope (2026-07-11: Mobile API Completion)

* Complete the mobile user-client API surface rather than stopping at visual screens. Every mobile action must use the existing versioned HTTP contract and show loading, authenticated, empty, and request-error states.
* Add missing user pages for authentication (registration and password reset), wallet operations (withdrawal, ledger/history, quick recharge), trade lifecycle operations (spot orders/cancel; margin positions/close/cancel; leverage and margin-mode settings), swap, public news, KYC, security, invitations, and the platform's user-facing product modules (seconds contracts, launchpad, earn, loan, and prediction markets).
* User-facing product pages may use focused mobile workflows, but their list/detail/quote/order/management actions must call the same backend endpoints used by the PC client. Never expose admin, staff, or agent endpoints through the mobile application.
* Keep sample market data confined to offline public-market rendering. Authenticated financial actions, balances, addresses, orders, positions, and product subscriptions must never be fabricated.

## Added Scope (2026-07-11: Mobile Visual Quality)

* Raise the mobile client from a functional prototype to a production-oriented exchange interface: establish consistent tactile surfaces, hierarchy, spacing, touch feedback, empty/loading/error states, compact financial typography, and deliberate white/dark trading contexts.
* Use the supplied OKX-like references for information density and mobile interaction patterns only. Keep Hippo branding, original component composition, original copy, and original visual assets; do not replicate third-party trademarks, logos, or proprietary layouts.
* Verify both 390px mobile and wide H5 constrained-container renderings after visual-system changes. No horizontal overflow, clipped controls, accidental visual overlap, or fake financial data is acceptable.

## Added Scope (2026-07-12: Mobile Navigation And Localization)

* Main bottom-navigation tabs must replace the active top-level route instead of stacking tab history. Detail workflows keep normal forward history, while reusable page headers provide a deterministic fallback route when no usable in-app history exists.
* The trade pair selector must return the selected symbol to the trade route. Normal market-list rows must continue to open market details, so picker mode is explicit and does not alter the public market-browsing flow.
* Route changes must handle scroll restoration, login redirects, expired sessions, and lightweight forward/back visual transitions without stale page state or blank intermediate screens.
* The mobile client must support persisted runtime language switching, initially aligned with the PC client locales: Simplified Chinese (`zh-CN`) and English (`en`). Fixed mobile UI copy, accessibility labels, validation feedback, and global navigation must react immediately; localized public-content requests must send the active locale when the backend contract supports it.
* Correct the mobile visual defects found during route QA: browser-default focused-link outlines, clipped overview metrics, safe-area/header offsets, modal keyboard bounds, and narrow/wide H5 layout consistency.

### Mobile Navigation And Localization Acceptance Criteria

* Switching `home -> markets -> assets` through the bottom bar does not leave the previous main tabs in browser history.
* Selecting a symbol from the trade pair picker navigates to `/trade/:symbol`; selecting the same row from the normal markets page navigates to `/markets/:symbol`.
* Directly opened detail routes always have a working fallback back destination.
* Changing the language updates the visible shell and feature-page copy immediately, updates `document.documentElement.lang`, and survives reloads.
* Mobile type-check, unit tests, production build, and 390px plus wide-H5 visual checks pass without horizontal overflow.

## Out of Scope (explicit)

* Deeply rewriting every business workflow in one task.
* Changing public API contracts unless needed to preserve behavior.
* Reworking existing desktop frontend/admin UI as part of the initial backend architecture slice, except for the standalone mobile client added above.
* Adding comments to every function mechanically.

## Technical Notes

* Existing `tests/` layout is already the preferred place for integration and domain tests.
* `src/modules/spot/mod.rs` already contains domain-ish types and services, so spot may be a useful first migration candidate.
* `src/modules/margin/routes.rs` is newly expanded by user-action work and may benefit from DDD separation, but it is also risk-sensitive because wallet and ledger mutations are involved.
* `src/modules/admin/routes.rs` is the largest file, but it spans many admin resources and may be better split after bounded-context conventions are established.
* Existing `.trellis/spec/backend/directory-structure.md` is a placeholder and should be filled with the agreed architecture.
* This task establishes the backend-wide DDD structure and test separation without moving risk-sensitive route logic yet. Future tasks should migrate each context's business workflows from `routes.rs` into `application`, `service`, `domain`, `repository`, and `infrastructure` layer files behind the stable public route API.
* The first real extraction slice now covers public countries, platform brand config, and public news. These contexts demonstrate the intended pattern: routes call application functions, application orchestrates use cases, infrastructure owns SQL, domain owns pure validation/search rules, and presentation owns API DTOs.
* The second extraction slice covers `risk` and `security`: risk evaluation is now fully in `domain`; security policy/TOTP pure rules moved into `security/domain.rs`; security policy, 2FA settings, login challenge, and user security SQL moved into `security/infrastructure.rs`; security verification use cases moved into `security/application.rs`. `security.rs` is now a compatibility module that re-exports the stable public API.
* The architecture was expanded after user feedback to include explicit `repository.rs` and `service.rs` files. Repository contracts should sit between domain/application/service and infrastructure; reusable business services should sit below application use-case orchestration.
* The auth core extraction now keeps `src/modules/auth/mod.rs` focused on shared auth models, token/hash helpers, request extractors, and compatibility re-exports. `AuthRepository` moved to `auth/repository.rs`; `MySqlAuthRepository` moved to `auth/infrastructure.rs`; `AuthService` plus project refresh-token Redis helpers moved to `auth/service.rs`.
* The auth application extraction moved registration-with-email-code, registration email-code sending, password reset code sending, purpose-based email verification, and password reset session revocation orchestration into `auth/application.rs`. The concrete SQL writes for verified-user insert, email-verification status changes, password update, and refresh-token revocation remain in `auth/infrastructure.rs`.
* The auth login 2FA extraction moved login policy lookup, username-login credential verification, 2FA challenge creation, TOTP verification, challenge consumption, and login-2FA reset orchestration into `auth/application.rs`. `routes.rs` now maps `UserLoginOutcome` to existing presentation DTOs without changing public payloads.
* The auth config extraction moved register/login config lookup and registration invite-code policy gating into `auth/application.rs`. `routes.rs` now maps `RegisterConfig`/`LoginConfig` to existing presentation DTOs and no longer imports `security::load_security_policy`.
* The auth actor extraction moved admin registration, admin login, agent login, scoped refresh-token exchange, and the explicit agent-registration rejection boundary into `auth/application.rs`. `routes.rs` now maps those use cases to the existing `TokenResponse` payloads and no longer calls `AuthService` directly.
* The first user extraction moved profile DTOs into `user/presentation.rs`, profile SQL and username persistence/audit helpers into `user/infrastructure.rs`, and profile/username use-case orchestration into `user/application.rs`. `user/routes.rs` still owns the wider legacy routes, but profile and username handlers are now thin.
* The user 2FA extraction moved 2FA status DTOs into `user/presentation.rs`, account-label lookup into `user/infrastructure.rs`, and TOTP setup/confirmation plus login-2FA policy orchestration into `user/application.rs`. The later password/security extraction also moved 2FA reset email-code verification into the application layer.
* The user email-bind extraction moved bind-email DTOs into `user/presentation.rs`, pure required-string and verification-expiry rules into `user/domain.rs`, email validation/code generation constants and helpers into `user/service.rs`, email-verification repository records into `user/repository.rs`, bind-email SQL helpers into `user/infrastructure.rs`, and send/confirm orchestration into `user/application.rs`.
* The user password/security extraction moved login-password and fund-password DTOs into `user/presentation.rs`, user password and email-verification repository records into `user/repository.rs`, password/fund-password validation plus email-code purpose constants into `user/service.rs`, password/fund-password SQL helpers into `user/infrastructure.rs`, and password change, token re-issue, fund-password create/change/reset, 2FA reset email-code verification, and verified-email reset-code delivery into `user/application.rs`.
* The user third-party binding extraction moved binding DTOs into `user/presentation.rs`, provider/identifier/display-name validation into `user/service.rs`, binding list/upsert SQL into `user/infrastructure.rs`, and policy gating plus audit orchestration into `user/application.rs`. `user/routes.rs` now keeps only the transport boundary for `/user/third-party-bindings`.
* The user KYC extraction moved user-side KYC status and submission orchestration into `user/application.rs`, reusing the existing `kyc` module for KYC config/submission validation and persistence while keeping `user/routes.rs` as a thin HTTP boundary. The KYC module's own DDD migration remains a future slice.
* The user referral extraction moved referral request/response DTOs into `user/presentation.rs`, invite/referral persistence records into `user/repository.rs`, invite-code generation and normalization into `user/service.rs`, referral SQL helpers into `user/infrastructure.rs`, and referral-code repair, bind-code, and my-invites orchestration into `user/application.rs`. Existing admin/auth code now imports invite-code generation from `user/service.rs` instead of `user/routes.rs`.
* The final user route cleanup moved avatar response DTOs into `user/presentation.rs`, avatar URL persistence into `user/infrastructure.rs`, avatar upload orchestration into `user/application.rs`, and the 2FA setup user-existence check into `setup_user_two_factor`. `user/routes.rs` now only performs extraction, multipart parsing, application calls, and response wrapping.
* The first KYC module split moved public DTOs, filters, and change records into `kyc/presentation.rs`, and moved KYC audit JSON plus identity-number masking into `kyc/service.rs`. `kyc.rs` still re-exports those symbols so existing admin/user call sites do not need API-path changes.
* The KYC infrastructure split moved KYC row structs, config bootstrap SQL, config/submission select SQL, QueryBuilder list filtering, user KYC locks, submission insert/update, and user-level update helpers into `kyc/infrastructure.rs`. The root `kyc.rs` module remains the stable public façade for validation and transaction orchestration.
* The KYC domain split moved `KycCountryDocumentTypeRule` and the pure validation/normalization rules into `kyc/domain.rs`. `kyc/presentation.rs` re-exports the rule type so external JSON contracts and existing import paths remain stable.
* The wallet read-query split moved wallet account, ledger, deposit asset, deposit network, and withdraw asset DTOs into `wallet/presentation.rs`, read use cases into `wallet/application.rs`, and the corresponding SQL/select row mapping into `wallet/infrastructure.rs`.
* The wallet deposit-address split moved `DepositAddressRequest`/`DepositAddressResponse` into `wallet/presentation.rs`, address-pool SQL helpers into `wallet/infrastructure.rs`, and the lock/assign/load transaction into `wallet/application.rs`.
* The wallet withdrawal split moved `CreateWithdrawalRequest`/`WithdrawalRequestResponse` into `wallet/presentation.rs`, withdrawal-fee loading and request insertion into `wallet/infrastructure.rs`, and request validation, server-side fee override, security verification, and response assembly into `wallet/application.rs`. `wallet/routes.rs` now keeps only HTTP extraction/auth/pool lookup for the wallet user endpoints.
* The first spot read-query split moved spot request/response DTOs into `spot/presentation.rs`, user/admin order query use cases and admin trade query use cases into `spot/application.rs`, and the corresponding read-only QueryBuilder SQL into `spot/infrastructure.rs`. The higher-risk order insertion, wallet freeze, and fill settlement transactions remain in `spot/routes.rs` for later transaction-focused slices.
* The spot cancellation split moved admin cancel reason validation into `spot/presentation.rs`, cancel use cases into `spot/application.rs`, cancel repository contracts into `spot/repository.rs`, domain state transition plus audit JSON helpers into `spot/service.rs`, and lock-order / remaining reservation / wallet unfreeze / order update / admin audit SQL into `spot/infrastructure.rs`. `spot/routes.rs` now keeps auth, pool lookup, application calls, and private event publication for user/admin cancellation.
* The spot execution-price split moved cached ticker loading/parsing into `spot/infrastructure.rs`, market reference-price tolerance and limit/stop-limit trigger predicates into `spot/service.rs`, and pre-insert execution-price selection into `spot/application.rs`. `spot/routes.rs` still owns the surrounding create-order transaction and event publication until the order insertion/freeze/triggered-fill slice is migrated.
* The spot idempotency replay split moved existing-order lookup by idempotency key into `spot/infrastructure.rs`, the persisted replay record into `spot/repository.rs`, request/record compatibility checks into `spot/service.rs`, and pre-insert replay orchestration into `spot/application.rs`. The insert-duplicate branch still lives in `spot/routes.rs` while order insertion and wallet freeze remain there, but it now reuses the same service matching rules.
* The first margin user-action split moved transfer/settings DTOs into `margin/presentation.rs`, wallet transfer and user-setting persistence helpers into `margin/infrastructure.rs`, and the transfer/leverage/margin-mode transaction orchestration into `margin/application.rs`.
* The margin position lifecycle split moved `MarginPositionResponse` into `margin/presentation.rs`, close/cancel/bulk close/bulk cancel use cases into `margin/application.rs`, and the corresponding SQL/Redis helpers into `margin/infrastructure.rs`. Routes now keep auth, pool lookup, application calls, and private event publication for these lifecycle endpoints.
* The margin open-position split moved `OpenMarginPositionRequest`/`OpenMarginPositionResponse` into `margin/presentation.rs`, idempotency/product/leverage/margin-mode/open-transaction orchestration into `margin/application.rs`, and active product locking, cached entry price, position insert, wallet collateral debit, ledger writes, and wallet-scope update into `margin/infrastructure.rs`.
* The margin read-query split moved product list, user position list/detail, margin wallet list, admin position history/detail, admin interest summary, and risk snapshot response DTOs into `margin/presentation.rs`; status normalization and risk snapshot assembly into `margin/application.rs`; and the corresponding QueryBuilder SQL plus Redis risk ticker lookup into `margin/infrastructure.rs`.
* The margin admin product split moved create/update/status request DTOs into `margin/presentation.rs`, product field validation and admin transaction orchestration into `margin/application.rs`, audit JSON mapping into `margin/service.rs`, and product insert/update/status SQL plus product audit writes into `margin/infrastructure.rs`. `margin/routes.rs` now keeps only transport/auth/pool handling for the margin context.
* The market module façade split moved market symbols, provider enums, ticker/depth/kline/trade snapshots, kline query values, and market event types into `market/domain.rs`; Redis cache entries, market Redis key generation, Mongo kline collection naming/filter helpers, and provider adapter/ingestion worker code now live in `market/infrastructure.rs`. `market/mod.rs` is now a thin compatibility façade that re-exports the stable public API, including `market::adapters`.
