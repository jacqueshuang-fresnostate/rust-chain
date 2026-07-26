# Research: Financial UX Guardrails for an Expressive Mobile Redesign

- Query: Research design constraints and best practices for visually expressive mobile crypto/financial interfaces inspired by OKX while keeping spot, contract, asset, authentication, and financial form flows legible and trustworthy. Focus on motion safety, reduced-motion, touch targets, data density, color semantics, page transitions, and bitmap/canvas usage.
- Scope: mixed
- Date: 2026-07-26

## Findings

### Executive conclusion

The redesign should use a two-zone model:

1. **Expressive discovery zone** — Home, market discovery, product discovery, logged-out promotion, and empty-state editorial areas may use oversized type, asymmetry, bitmap art, restrained Canvas fields, and richer transitions.
2. **Calm transaction zone** — Spot, contract, assets, authentication, KYC/security, deposit, withdrawal, transfer, order review, and all amount-entry surfaces must use stable geometry, neutral backgrounds, conventional controls, explicit labels, and minimal motion.

The award-level character should come from composition, typography, art direction, and transitions between zones—not from making prices, risk controls, credentials, or irreversible financial actions visually novel.

The OKX-inspired qualities worth adopting are information hierarchy and workflow completeness: account value first, strong funding actions, dense market lists, clearly separated spot/derivatives semantics, and risk data next to the order action. Do not copy OKX branding, proprietary illustrations, copy, or exact page layouts.

### Files found

| File | Relevance |
| --- | --- |
| `mobile/src/App.vue` | Production route-transition host and current reduced-motion handling. |
| `mobile/src/core/navigation.ts` | Depth-based forward/back/fade transition selection. |
| `mobile/src/router/index.ts` | Production route depth, fallback, auth, asset, and trading route structure. |
| `mobile/src/styles/base.css` | Production color tokens, focus states, touch dimensions, numeric style, and shared form controls. |
| `mobile/src/components/AppBottomNav.vue` | Production five-tab navigation and hit-area implementation. |
| `mobile/src/views/TradeView.vue` | Production spot/contract order entry, mode semantics, leverage, order book, and submit behavior. |
| `mobile/src/components/OrderBookPanel.vue` | Dense bid/ask presentation and market color semantics. |
| `mobile/src/components/MobileMarketChart.vue` | Production functional chart integration and touch gestures. |
| `mobile/src/views/AssetsView.vue` | Asset hierarchy, funding actions, transfer dialog, and financial feedback. |
| `mobile/src/views/DepositDetailView.vue` | QR/address, network, minimum, timing, memo, and deposit warnings. |
| `mobile/src/views/WithdrawView.vue` | Withdrawal validation, fee/arrival estimate, 2FA, and direct submit flow. |
| `mobile/src/views/LoginView.vue` | Two-step login, autofill metadata, accessible labels, and auth feedback. |
| `mobile/src/views/LoginTwoFactorView.vue` | OTP/reset flow and one-time-code metadata. |
| `mobile/sites-prototype/app/page.tsx` | Current six-tab visual prototype, separate spot/contract surfaces, ambient Canvas, chart Canvas, and simulated trading. |
| `mobile/sites-prototype/app/secondary-pages.tsx` | Prototype financial-ticket review pattern and deposit/withdrawal warning patterns. |
| `mobile/sites-prototype/app/globals.css` | Prototype visual tokens, expressive animation, bitmap stage art, and global reduced-motion rule. |
| `.trellis/tasks/07-26-mobile-pencil-redesign/research/design-direction.md` | Earlier “signal field” direction and initial interaction constraints. |
| `.trellis/tasks/07-26-mobile-pencil-redesign/research/okx-mobile-reference.md` | Earlier OKX mobile layout study and anti-copying boundary. |

### Existing code patterns and implications

#### What is already strong

- Production navigation already derives forward/back/fade from route depth (`mobile/src/core/navigation.ts:37-42`) and maps financial/auth detail screens to explicit depths and fallback routes (`mobile/src/router/index.ts:41-74`). Preserve this mental model.
- Production route movement is short and restrained: 150–180 ms, 8–12 px, opacity plus translation (`mobile/src/App.vue:36-42`). It is already much safer for financial surfaces than the prototype’s 480 ms translate-and-blur entrance.
- Production route transitions stop when the operating system requests reduced motion (`mobile/src/App.vue:43-45`).
- Shared production icon buttons are 44×44 px, normal buttons are at least 46 px high, and inputs are 48 px high (`mobile/src/styles/base.css:60-100`).
- Focus-visible styling is explicit and separate from ordinary focus (`mobile/src/styles/base.css:32-33`).
- Numeric content has a shared tabular-number utility (`mobile/src/styles/base.css:120`), which should become mandatory for balances, prices, percentages, fees, leverage, and quantities.
- Production bottom navigation provides at least 50 px vertical hit areas and uses `replace`, avoiding accidental history stacking (`mobile/src/components/AppBottomNav.vue:23-35`, `mobile/src/components/AppBottomNav.vue:58-69`).
- Spot and contract action labels are semantically distinct in production: buy/sell versus long/short (`mobile/src/views/TradeView.vue:45-48`, `mobile/src/views/TradeView.vue:202-213`).
- The newer prototype goes further by separating spot and contract into independent primary destinations (`mobile/sites-prototype/app/page.tsx:151-158`) and by exposing mark price, funding rate, margin mode, leverage, estimated margin, and explicit “open long/open short” wording (`mobile/sites-prototype/app/page.tsx:793-933`).
- Asset actions are grouped predictably, with a prominent total and stable 65–74 px rows (`mobile/src/views/AssetsView.vue:142-152`, `mobile/src/views/AssetsView.vue:173-176`).
- Deposit details pair QR/address with text, copy action, network, minimum, estimated arrival, memo, and explicit wrong-network warnings (`mobile/src/views/DepositDetailView.vue:58-70`).
- Login uses labels, correct `autocomplete` values, a password reveal control, text errors, and a two-step structure (`mobile/src/views/LoginView.vue:100-134`). The 2FA input uses `autocomplete="one-time-code"` (`mobile/src/views/LoginTwoFactorView.vue:96-101`).
- The prototype’s ambient Canvas is non-interactive, behind content, and hidden from assistive technology (`mobile/sites-prototype/app/page.tsx:160-232`; `mobile/sites-prototype/app/globals.css:183-207`).
- The prototype caps Canvas device-pixel ratio at 2 and uses `requestAnimationFrame`, cleanup, and resize handling (`mobile/sites-prototype/app/page.tsx:169-229`), avoiding unbounded high-DPR buffers.
- Prototype financial tickets already demonstrate a better two-step “quote then review then confirm” pattern with visible expiry and invalidation rules (`mobile/sites-prototype/app/secondary-pages.tsx:560-625`).

#### Current risks to address in the redesign

- Production’s primary button uses the positive-market green for every primary action, including login and withdrawal (`mobile/src/styles/base.css:86-89`). This conflates “continue/confirm” with “price up/buy/success.” Introduce a neutral brand/action token; reserve positive and negative tokens for market direction and explicit buy/sell semantics.
- Some dense production controls are visually and interactively below the preferred mobile target size: 31 px percentage chips, 34–35 px leverage/mode controls, and small inline links (`mobile/src/views/TradeView.vue:228-232`). Even when they satisfy WCAG’s 24 px minimum, they fall short of the 44 pt iOS and 48 dp Android recommendations for frequent financial actions.
- Production applies reduced motion only to route transitions. Refresh spinners remain infinite animations in multiple pages, including assets (`mobile/src/views/AssetsView.vue:137`, `mobile/src/views/AssetsView.vue:177`). Add a global reduced-motion contract for all keyframes and JS animation loops.
- The prototype ambient Canvas runs continuously for users who have not requested reduced motion and has no pause/hide control (`mobile/sites-prototype/app/page.tsx:185-227`). Ambient movement lasting over five seconds alongside functional content needs a user control, or it should stop after a short intro.
- The prototype checks `prefers-reduced-motion` only when the Canvas effect mounts (`mobile/sites-prototype/app/page.tsx:169`). It does not subscribe to preference changes. The implementation should listen for `MediaQueryList` changes or remount safely when the preference changes.
- The prototype’s normal page entrance is 480 ms and animates blur plus vertical movement (`mobile/sites-prototype/app/globals.css:295-315`). Apple specifically advises avoiding depth/blur transitions and replacing axis movement with fades for people who reduce motion. On transaction surfaces this animation is too theatrical even for the normal-motion mode.
- Production keys the routed component by `currentRoute.fullPath` (`mobile/src/App.vue:26-29`). Query-only changes such as spot/contract mode can remount and animate the entire financial screen. Same-surface filters, order types, leverage choices, and mode controls should update in place without page-level choreography or losing entered values.
- The production contract screen shows leverage, margin mode, and margin asset but does not show estimated liquidation price or a pre-submit risk summary (`mobile/src/views/TradeView.vue:198-213`). This is below the risk-information completeness shown in current OKX derivatives guidance.
- Production withdrawal validates values, then submits directly (`mobile/src/views/WithdrawView.vue:61-93`, `mobile/src/views/WithdrawView.vue:99-120`). It shows fee and estimated arrival but lacks a distinct review/confirm step containing the final address, network, gross debit, fee, net receive, and security state.
- The prototype functional price chart has only an `aria-label` (`mobile/sites-prototype/app/page.tsx:235-300`), and the production Lightweight Charts container exposes attribution but no equivalent text/table summary (`mobile/src/components/MobileMarketChart.vue:94-102`). Functional Canvas/chart content needs a semantic alternative, not only an accessible name.
- The production order book relies strongly on red/green rows and depth bars (`mobile/src/components/OrderBookPanel.vue:16-49`). Add persistent bid/ask text headings or shape/direction cues so hue is not the sole distinction.

## Guardrail framework

### 1. Surface classification

Use the following classification before designing a screen:

| Surface | Expressiveness budget | Motion budget | Image/Canvas rule |
| --- | --- | --- | --- |
| Home / discovery / editorial | High | One ambient system plus short entrances | Allowed behind content; never required for navigation or meaning |
| Markets list / asset overview | Medium | Small value-change feedback; no persistent ornament near rows | Decorative only; charts must have text values |
| Spot order entry | Low | Direct state changes; short panel transition only | No ambient imagery behind form/order book |
| Contract order entry / positions | Very low | No parallax, blur, orbit, or count-up animation | Charts only; risk and controls remain semantic DOM |
| Deposit / withdrawal / transfer / earn / loan | Very low | Step transition or sheet transition only | No bitmap/Canvas behind fields, review, warnings, QR, or confirmation |
| Login / registration / 2FA / KYC / security | Very low | Optional fade between steps | Brand mark allowed; no ambient animation or visually noisy background |
| Success receipt / completed state | Medium | One finite confirmation animation, then static | Illustration allowed after authoritative receipt details |

Functional surfaces must remain complete and usable when all decorative layers fail to load, Canvas is unsupported, CSS backgrounds disappear, animation is disabled, or the app is used at 200% text zoom.

### 2. Motion safety and reduced motion

#### Normal-motion rules

- Use motion to explain hierarchy or direction, not to advertise urgency.
- Page transitions: target 150–220 ms, opacity plus at most 8–16 px translation. Avoid blur, zoom, rotation, large parallax, spring overshoot, and simulated camera depth on transaction/auth surfaces.
- Same-screen state changes—buy/sell, order type, percentage, leverage, margin mode, tab, validation, price update—must not trigger a page transition.
- Ambient motion may appear on Home/discovery only. It should either:
  - stop after no more than five seconds, or
  - provide an always-reachable “pause visual motion” control that persists as a preference.
- Do not animate every market tick. Keep layout fixed, use tabular numerals, and if a value-change cue is necessary use a single low-contrast background tint that fades once. Never flash, pulse, scale, or shake a price.
- Loading motion is status, not decoration. Pair a spinner with text, keep it local to the affected region, and avoid replacing the whole screen when stale-but-usable data exists.
- Critical errors, risk warnings, quote expiry, and irreversible-action states must remain visible until resolved; do not communicate them only with a timed toast.

#### Reduced-motion rules

- Honor both the OS preference and an in-app preference. Either preference must produce the calmer result.
- In reduced mode:
  - cancel all `requestAnimationFrame` loops and render one static frame;
  - disable parallax, blur, orbit, shimmer, count-up, auto-scrolling tickers, pulse, shake, and scale feedback;
  - replace directional page movement with an immediate swap or a very short opacity crossfade;
  - show static progress indicators plus text instead of rotating spinners where practical;
  - stop animated bitmaps and use a still first frame;
  - keep success/error feedback visible without celebratory motion.
- Subscribe to runtime changes in `matchMedia("(prefers-reduced-motion: reduce)")`; do not read the setting only once.
- Pause nonessential animation when the document is hidden or the animated region is offscreen.

#### Acceptance checks

- With system Reduce Motion enabled before launch, no ambient line, ticker, spinner, blur, parallax, or sheet movement continues indefinitely.
- Turning Reduce Motion on while the app is open stops active Canvas/CSS animation without reload.
- A user can complete spot, contract, deposit, withdrawal, transfer, login, and 2FA with animation globally disabled.
- No content flashes more than three times in one second.

### 3. Touch targets and gesture safety

- Product baseline: **44×44 CSS px minimum on iOS/H5 and 48×48 dp on Android** for all frequent or consequential controls.
- A chip may look smaller, but its invisible hit box must reach the baseline without overlapping a neighboring target.
- Maintain 8 px minimum separation between adjacent financial controls; prefer 12 px around icon-only actions and destructive/irreversible actions.
- Percentage chips, leverage choices, margin-mode controls, order-type tabs, “max,” fee/network selectors, address-book actions, password visibility, and OTP resend all count as consequential controls.
- Never make swipe, drag, long-press, or chart gestures the only path to an action. Provide visible buttons for back, close, cancel, edit, and chart timeframe selection.
- Chart pinch/drag must not steal vertical page scrolling. The current production chart already disables vertical touch drag while allowing horizontal drag and pinch (`mobile/src/components/MobileMarketChart.vue:61-62`); preserve that behavior.
- Keep the final submit/confirm action above the safe-area inset and visible when the keyboard opens. It must not overlap the bottom navigation.
- Require pointer-up/click completion; moving off the target before release must cancel the action.

#### Acceptance checks

- Automated or manual hit-area overlay finds no consequential target below platform baseline.
- At 320 px width and 200% text size, targets do not overlap and labels remain distinguishable.
- One-handed testing at 390×844 confirms primary actions are reachable without accidentally activating adjacent buy/sell or percentage controls.

### 4. Data density and numeric hierarchy

Dense is acceptable when hierarchy is stable. Compactness must come from alignment and progressive disclosure, not tiny type or omitted labels.

#### Global numeric rules

- Use tabular numerals for all changing numeric values.
- Keep a consistent right edge for comparable prices, quantities, balances, and percentages.
- Pair every amount with an asset/unit in the same visual group.
- Show signs for direction (`+2.41%`, `−1.08%`) and labels for action (“买入”, “卖出”, “开多”, “开空”); do not depend on green/red.
- Never truncate a user-entered amount, destination address, network, fee, liquidation price, or final receive amount in a review step.
- Use locale-aware grouping for display values, but keep editable numeric inputs ungrouped and preserve exact decimal precision.
- Reserve 11–12 px text for tertiary labels and table metadata only. Order actions, risk labels, field labels, and amounts must remain comfortably readable.
- Prevent layout shift by reserving width for live values and keeping row heights stable.

#### Spot surface minimum information

Before submit, keep visible or one direct tap away:

- pair and spot label;
- current price and 24h direction;
- order type, side, price, quantity, and unit;
- available balance;
- estimated spend/receive;
- fee or a clear fee rule;
- market-order slippage/price-uncertainty warning where applicable;
- explicit final action label containing side and asset.

Spot and contract may share components, but they must never share ambiguous wording or preserve stale values when switching modes.

#### Contract surface minimum information

Before opening a position, show:

- contract/instrument type;
- open long/open short;
- order type;
- margin mode;
- leverage;
- quantity unit (“张” versus coin amount);
- mark price;
- estimated initial margin;
- estimated liquidation price or an explicit “calculated after…” explanation if unavailable;
- funding rate and next funding context where relevant;
- available margin;
- fee estimate;
- TP/SL access;
- risk warning adjacent to the submit region;
- link to current positions/orders.

Do not hide liquidation, margin, or fee information inside a decorative card carousel or horizontal scroller.

#### Asset surface minimum information

- Total portfolio value is the primary number.
- Separate available, frozen/locked, funding/spot, contract margin, earn, and debt semantics. Avoid presenting them as one undifferentiated “balance.”
- Deposit, withdraw, transfer, and ledger remain conventional, high-visibility actions.
- Positive/negative PnL is visually distinct from deposit/withdraw success states.
- Loading or partial API failure must identify which account scope is stale rather than replacing all values with zero.

### 5. Color semantics and trust

Use a neutral-first semantic palette:

| Semantic role | Treatment |
| --- | --- |
| Primary navigation / continue / confirm | Brand-neutral accent or high-contrast ink; not market green |
| Price up / bid / buy / open long | Positive green plus sign/text/icon |
| Price down / ask / sell / open short | Negative red/coral plus sign/text/icon |
| Success receipt | Success token plus check icon and explicit text |
| Error / destructive action | Error token plus icon, border, and text |
| Warning / liquidation / network mismatch | Amber/yellow plus warning icon and text |
| Information / focus | Cyan/blue, reserved for focus, links, and informational emphasis |
| Disabled | Neutral low-emphasis surface and text; never merely reduced opacity below readable contrast |

Additional rules:

- Color can reinforce meaning but cannot be the only signal.
- Normal text must meet at least 4.5:1 contrast; large text at least 3:1; focus indicators and meaningful control boundaries need at least 3:1 against adjacent colors.
- Avoid luminous green as a full-page brand wash on trading or asset screens; it makes gains and buy actions harder to parse.
- Avoid celebratory color or confetti after deposits/withdrawals until the authoritative state is confirmed. “Submitted,” “broadcast,” and “confirmed” are different statuses.
- Theme changes must preserve semantics: up is still up, warning is still warning, and disabled controls remain distinguishable in both themes.
- Charts should supplement hue with candle geometry, line style, labels, or direct numeric summaries.

### 6. Page transitions and navigation

- Preserve the existing depth model:
  - primary-tab to primary-tab: no directional slide; direct swap or short fade;
  - list to detail: subtle forward motion;
  - detail back to parent: subtle reverse motion;
  - auth completion: replace history and avoid a transition back into a completed credential step.
- Keep bottom tabs stable during primary navigation. Hide them on detail/auth/financial flows as the route contract already does (`mobile/src/router/index.ts:43-74`).
- Do not animate query-only changes as navigation. Spot/contract mode, pair, order type, filters, and chart timeframe should preserve local context unless the product intentionally opens a new destination.
- Never transition a transaction form while submit is in flight.
- Avoid skeleton-to-content movement that changes the submit button’s position.
- Sheet/dialog transitions should be 180–240 ms maximum in normal mode, maintain a stable backdrop, trap focus, restore focus on close, and become immediate in reduced mode.
- Back must preserve unfinished form state when safe. If leaving would discard an entered withdrawal/address/order, ask for confirmation in plain language.

### 7. Financial forms and irreversible actions

Apply WCAG 2.2 error-prevention principles to every order, withdrawal, transfer, loan, earn subscription, and security change:

1. Validate and identify the exact field in text.
2. Preserve entered values after a validation/server error.
3. Show a review state before finalizing, unless the action is safely reversible.
4. Allow correction from review without re-entering unchanged data.
5. Prevent duplicate submission and show an authoritative pending state.
6. Return a receipt/status with a durable identifier when available.

#### Withdrawal review must show

- full asset and network;
- destination type and address (with enough leading/trailing characters to verify; full value expandable/copyable);
- memo/tag if required;
- withdrawal amount;
- network fee;
- total wallet debit;
- estimated receive amount;
- minimum/maximum and remaining limit where relevant;
- security/allowlist state;
- explicit “Confirm withdrawal” action;
- irreversible/wrong-network warning immediately above the final action.

The current production flow shows fee and estimated arrival but submits directly; the prototype review pattern is the better model to carry forward.

#### Deposit details must show

- asset, network, destination account;
- QR and exact copyable address;
- memo/tag with equal visual prominence when required;
- minimum deposit and confirmation requirement;
- wrong-asset/wrong-network loss warning before the address is used;
- no decorative texture behind QR codes or addresses.

#### Authentication rules

- Allow password-manager autofill and paste.
- Keep standard autocomplete tokens (`email`, `username`, `current-password`, `new-password`, `one-time-code`).
- Never split OTP into boxes that block paste or force character-by-character navigation unless the composite remains one accessible input.
- Do not add puzzle-like cognitive tests without a compliant alternative.
- Use explicit text for errors; do not shake the form.
- Keep brand art static and peripheral. Credentials, MFA, recovery, KYC, and security changes receive no ambient Canvas.
- Avoid time-boxed notices. OTP countdowns may update, but resend remains clearly disabled/enabled with text.

### 8. Bitmap imagery and Canvas

#### Bitmap placement

- Use bitmap artwork on the desktop preview stage, Home hero, product discovery, campaign/editorial cards, and optional success receipts.
- Do not place bitmap art behind order forms, order books, balances, addresses, QR codes, credentials, KYC uploads, risk warnings, or confirmation details.
- Decorative imagery should be CSS background imagery or use empty alt text and must disappear without loss of information.
- Informative imagery needs concise alt text; complex charts/diagrams need an adjacent text summary or data representation.
- Never render prices, labels, fee terms, addresses, risk copy, or buttons into a bitmap.
- Provide intrinsic dimensions/aspect ratio to prevent layout shift; ship responsive sizes and modern formats where supported.
- Treat art as an optional enhancement: failure to decode must not change contrast, hit targets, or functional layout.
- Avoid animated GIFs. Prefer CSS/Canvas with explicit reduced-motion behavior, or provide a still alternative.

#### Ambient Canvas

Allowed only when all are true:

- `aria-hidden="true"` or `role="presentation"`;
- `pointer-events: none`;
- below functional content in stacking order;
- low enough opacity that text/control contrast is independent of it;
- no semantic data is conveyed;
- one static frame is meaningful in reduced mode;
- loop is cancellable, pausable, and stopped while hidden/offscreen;
- DPR and buffer dimensions are capped;
- no full-screen shadow blur or per-frame bitmap scaling;
- no effect on layout or scroll.

The current `SignalField` is close technically—hidden, noninteractive, DPR-capped, and cleaned up—but needs pause/runtime-preference handling and stricter placement away from transactional views.

#### Functional Canvas/chart

- A chart may use Canvas for performance, but must have semantic HTML next to it containing:
  - instrument;
  - timeframe;
  - latest price;
  - change;
  - high/low;
  - optional tabular OHLC values for the selected point;
  - a textual trend summary that does not overclaim.
- Canvas fallback content or an adjacent DOM summary is required; `aria-label` alone is not enough for detailed financial data.
- All chart settings and timeframe actions remain DOM controls with platform-size hit targets.
- Use `requestAnimationFrame` for animation, batch drawing work, cap DPR, resize deliberately, and avoid `shadowBlur` in repeated frames.
- Pause rendering when hidden; update only changed data rather than repainting decorative layers.
- A chart must not be the only source for exact price, liquidation level, order state, or warning.

### 9. Recommended visual direction

A safe award-level direction for this product is:

- **Identity:** editorial typography, asymmetric discovery compositions, confident black/white or graphite/off-white foundation, a single distinctive brand accent, and carefully commissioned bitmap art.
- **Market semantics:** familiar green/red restricted to data and trade side.
- **Transaction surfaces:** squared or lightly rounded modules, thin dividers, stable alignment, strong labels, and high-contrast totals.
- **Motion:** one recognizable ambient “signal” motif on discovery screens, then calm, almost instrument-like behavior in trading and finance.
- **Trust cues:** visible data source/live-vs-sample state, update timestamp, network/fee/risk summaries, clear account scopes, and durable receipts.
- **Originality boundary:** borrow OKX’s prioritization and workflow completeness, not its logo, exact component geometry, proprietary illustration language, wording, or screen composition.

## Flow-specific acceptance matrix

| Flow | Must remain visible/explicit | Must not happen |
| --- | --- | --- |
| Spot | pair, side, order type, amount/unit, available, estimate, fee/rule, final action | contract terminology, hidden market-order uncertainty, color-only side |
| Contract | long/short, margin mode, leverage, mark, funding, margin, liquidation/risk, positions | decorative motion near submit, hidden risk in carousel, “buy/sell” ambiguity |
| Assets | total, account scopes, available/frozen, deposit/withdraw/transfer/ledger | zeroing on partial failure, PnL color reused for generic actions |
| Deposit | asset/network/account, QR, exact address, memo, minimum, warnings | textured QR background, address only in image, hidden memo |
| Withdrawal | address/network, amount, fee, total debit, receive, review, 2FA, receipt | one-tap irreversible submit, disappearing warning, fee shown only after submit |
| Transfer | from/to scopes, available, amount, resulting balances, review | direction conveyed only by icon/color, silent duplicate submission |
| Login/register | labels, autofill/paste, password reveal, errors, recovery | ambient motion, shake, puzzle without alternative, blocked paste |
| 2FA/security | one-time-code support, alternative/recovery, persistent errors | forced manual transcription, auto-dismissed error, celebratory motion |

## External references

All external references were checked on 2026-07-26.

### Accessibility and interaction

- [WCAG 2.2](https://www.w3.org/TR/WCAG22/) — W3C Recommendation. Relevant criteria include contrast, use of color, reflow, non-text contrast, pause/stop/hide, animation from interactions, target size, error identification, error prevention for legal/financial/data actions, and accessible authentication.
- [Understanding Target Size (Minimum), WCAG 2.2 SC 2.5.8](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum) — Level AA requires at least 24×24 CSS px or sufficient spacing, while larger targets remain the best practice.
- [Apple Human Interface Guidelines: Accessibility](https://developer.apple.com/design/human-interface-guidelines/accessibility) — current platform guidance recommends 44×44 pt default iOS/iPadOS controls, adequate spacing, simple gestures, reduced automatic/repetitive motion, and replacing axis/depth/blur movement with calmer alternatives.
- [Android Developers: Make apps more accessible](https://developer.android.com/guide/topics/ui/accessibility/views/apps-views) — recommends at least 48×48 dp focusable/touch targets, with padding allowed to enlarge the hit area.
- [Understanding Pause, Stop, Hide, WCAG 2.2 SC 2.2.2](https://www.w3.org/WAI/WCAG22/Understanding/pause-stop-hide.html) — ambient movement over five seconds and auto-updating content shown alongside other content need user control unless essential.
- [Understanding Use of Color, WCAG 2.2 SC 1.4.1](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color) — hue cannot be the only visual means of conveying information or state.
- [Understanding Accessible Authentication (Minimum), WCAG 2.2 SC 3.3.8](https://www.w3.org/WAI/WCAG22/Understanding/accessible-authentication-minimum) — supports password managers, paste, and one-time-code completion; authentication cannot require an unsupported cognitive-function test.
- [MDN: `prefers-reduced-motion`](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/At-rules/%40media/prefers-reduced-motion) — current Web platform reference for honoring the operating-system motion preference.

### Images and Canvas

- [W3C WAI: Decorative Images](https://www.w3.org/WAI/tutorials/images/decorative/) — decorative images should be ignored by assistive technology; CSS backgrounds are appropriate when imagery carries no information.
- [MDN: Basic usage of Canvas](https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API/Tutorial/Basic_usage.) — Canvas needs fallback text/sub-DOM for meaningful content; purely presentational Canvas can use a presentation role.
- [MDN: Optimizing Canvas](https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API/Tutorial/Optimizing_canvas) — updated 2025-11-07; recommends `requestAnimationFrame`, DPR-aware sizing, batching, offscreen prerendering when useful, and avoiding repeated scaling/shadow blur.
- [web.dev: Responsive Images](https://web.dev/learn/design/responsive-images) — use responsive sources and treat purely decorative images as non-informative.

### Current OKX product references

- [OKX: How to trade X-Perps on mobile](https://www.okx.com/en-us/learn/how-to-trade-x-perps-on-mobile) — updated 2026-06-02; demonstrates that leverage, order type, TP/SL, margin, estimated liquidation price, positions, and account context belong in the mobile derivatives workflow.
- [OKX: How do I trade with different order types?](https://www.okx.com/en-us/help/how-do-i-trade-with-different-order-types) — current spot guidance distinguishes market/limit behavior and confirms amount/order review.
- [OKX: Introduction to spot mode](https://www.okx.com/en-eu/help/introduction-to-spot-mode-eea) — updated 2026-05-29; explicitly distinguishes spot from margin/perpetual modes and emphasizes available/max buy/max sell context.
- [OKX: How do I make a deposit? (app)](https://www.okx.com/en-us/help/how-do-i-make-a-deposit-app) — updated 2026-07-09; requires matching asset/network and exposes address, QR, destination account, minimum, confirmation count, and memo/tag where relevant.
- [OKX: How do I make a crypto withdrawal? (app)](https://www.okx.com/en-us/help/how-do-i-make-a-crypto-withdrawal-app) — updated 2026-07-08; shows destination/network/address/amount/fee followed by 2FA and confirmation.
- [OKX: Deposit and withdrawal fees](https://www.okx.com/en-us/help/do-i-need-to-pay-fees-for-deposit-and-withdrawal) — updated 2026-05-29; confirms that withdrawal fee logic and amount received can vary by product/app behavior and must be made explicit.

## Related specs

- `.trellis/spec/mobile/index.md` — mobile package entry point and required type-check/test/build quality gate.
- `.trellis/spec/mobile/navigation-and-localization.md:45-76` — router history, trade context, auth redirect, and localization contracts.
- `.trellis/spec/mobile/navigation-and-localization.md:130-134` — visual preview independence and 390 px overflow/navigation checks.
- `.trellis/spec/backend/spot-orders.md` — authoritative spot order semantics and validation behavior.
- `.trellis/spec/backend/margin-trading-actions.md:19-48` — market freshness, margin modes, leverage capability, wallet scope, cross-account risk, and error behavior.
- `.trellis/spec/backend/wallet-amount-precision.md:10-22` — amount precision and truncation contract.
- `.trellis/spec/backend/wallet-amount-precision.md:43-68` — tiered withdrawal fee source of truth and server-calculated fee contract.
- `.trellis/spec/backend/deposit-addresses.md:19-32` — asset/network compatibility and address-group validation.
- `.trellis/spec/backend/user-authentication.md:25-45` — login identifier and policy-gated authentication behavior.
- `.trellis/spec/backend/auth-sessions.md:28-51` — session refresh, expiry, scope, and logout behavior.
- `.trellis/spec/backend/platform-display-and-chart.md:25-33` — internal market-data source, Lightweight Charts, attribution, and renderer code-splitting.

## Implementation priority

1. Establish neutral/positive/negative/warning/info semantic tokens and stop using positive green as the universal primary action.
2. Define the discovery-zone versus transaction-zone component contract.
3. Centralize global reduced-motion behavior for CSS and JS, including runtime preference changes and pause control.
4. Raise all consequential hit areas to 44/48 baseline.
5. Add explicit review states for withdrawal and other irreversible financial forms.
6. Complete contract pre-submit risk information.
7. Add semantic DOM alternatives for Canvas/chart content.
8. Replace prototype blur/long entrances on functional routes with production-style restrained transitions.
9. Test 320 px, 390×844, 200% text, keyboard-open safe areas, dark/light themes, motion on/off, Canvas/images blocked, and slow/partial data states.

## Caveats / Not Found

- `python3 ./.trellis/scripts/task.py current --source` returned no session-scoped current task. The task path and exact output file were supplied explicitly in the research request and were used as the Trellis sub-agent fallback.
- The active task directory contains `task.json`, `implement.jsonl`, and `check.jsonl`, but no `prd.md` yet. This research therefore defines guardrails, not a final visual concept or implementation scope.
- Production `mobile/src` currently uses five primary tabs, while the newer standalone Sites prototype uses six and separates spot/contract. This research does not resolve which navigation model will ship; the redesign must declare its target before migration.
- OKX product pages vary by region, regulation, account state, and app version. References are used for hierarchy and workflow completeness, not as a pixel-accurate specification.
- WCAG’s 24×24 CSS px target-size criterion is a compliance floor, not the recommended product baseline. This document intentionally adopts the larger Apple/Android targets for a high-risk financial interface.
- No user research, color-vision simulation, screen-reader walkthrough, low-end-device performance profile, or real-money usability test was run in this research pass.
- Contrast ratios for the current prototype token combinations were not measured programmatically; all final light/dark token pairs require automated and manual contrast verification.
- The current production chart’s third-party library accessibility behavior was not exhaustively audited. An adjacent semantic summary is required regardless of library internals.
