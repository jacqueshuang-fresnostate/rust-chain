# Mobile Development Guidelines

> Executable conventions for the Vue 3 + Vite + Tauri mobile client in `mobile/`.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Navigation and Localization](./navigation-and-localization.md) | Router history, trade context, safe back behavior, and `vue-i18n` contracts | Active |
| [PWA, Theme, and Application Shell](./pwa-and-shell.md) | Web/Tauri build isolation, shell-only caching, persisted themes, root navigation, and message truthfulness | Active |
| [Backend Integration](./backend-integration.md) | Runtime URL selection, Vite proxying, auth refresh, WebSocket, and DTO adapter contracts | Active |

## Quality Check

Run from `mobile/` after navigation, localization, or shared UI changes:

```bash
npm run type-check
npm test
npm run build:pwa
npm run build:tauri
```

For changes that affect Tauri startup or dependencies, also build Android and iOS targets:

```bash
npm run tauri:android:build -- --debug --target aarch64 --apk
npm run tauri:ios:build -- --debug --target aarch64-sim --no-sign
```

## Icon-Only Control Contract

Circular icon-only buttons must center their SVG explicitly on both axes. Do
not rely on inherited `text-align` or the browser's default button layout.

```css
.icon-only-button {
  display: grid;
  place-items: center;
  padding: 0;
}
```

For shared control changes, verify the SVG and button bounding-box centers
match on both axes and preserve a minimum 44x44 touch target.

## Local Sites Prototype Surface Contract

Apply this contract to `mobile/sites-prototype/` secondary routes:

- Secondary headers use a business-domain label and route-specific context.
  Keep the third grid track for alignment, but an absent action must set an
  explicit empty state and remain visually hidden.
- Shared fields expose visible `focus-within`, invalid, disabled, unit, hint,
  and completion states without changing their dimensions. Validation styling
  must belong to the field that failed, not to unrelated workflow errors.
- Nested inputs inside a framed field must not draw a second focus outline.
  Move the visible keyboard and pointer focus treatment to the full field
  container, while preserving the field's validation color when focus and
  invalid states overlap.
- Light-theme controls must not communicate selection through text color
  alone. Segmented controls, quick amounts, and primary actions need distinct
  filled, bordered, or inset states; scope these selectors to the relevant
  control group so card buttons and utility toggles retain their own semantics.
- Consequential local mutations use the shared bottom confirmation dialog.
  It must provide `role="dialog"`, `aria-modal`, labelled title/summary,
  overlay and Escape dismissal, a contained Tab loop, background scroll lock,
  and focus restoration.
- Dangerous confirmations initially focus the cancel action. Submitting or busy
  state must not restore focus behind an open dialog.
- Backend-style status values must be mapped to user-facing Chinese labels.
  Preserve unknown values only where the mobile API localization contract
  explicitly requires source visibility.
- Use Lucide icons only, no emoji, and keep all interactive targets at least
  44x44 CSS pixels.

For root navigation, seconds trading, and sticky headers in the Sites
prototype:

- Keep spot, contract, and seconds trading as separate operational surfaces.
  Seconds trading must use its typed protected route and must not be folded into
  either root trading column.
- A shaped root navigation may raise one protected destination above the
  navigation body, but all root destinations must remain visible and the full
  control must fit from 320px through 448px without horizontal page overflow.
- The v16 root navigation is 84px plus the bottom safe area. Its seven item
  rails use a 66px minimum height, and the raised Seconds control is 48px.
- Seconds trading remains a deterministic local prototype. It must expose pair,
  reference price, round, direction, duration, amount, payout, balance,
  confirmation, and session feedback while explicitly avoiding real orders or
  external side effects.
- Root and secondary headers must use an opaque sticky layer above route
  transitions and scrolling content. Content stacking contexts must remain
  below the header layer.
- A route host must not create a stacking context that traps sticky headers
  below the shaped root navigation. During transitions, the entering route is
  above navigation and the leaving route is demoted to the content layer so an
  old header cannot cover the new route.
- Light-theme border tokens must use the shared cool-neutral family. Do not
  reintroduce the retired `#0b1811` / `rgba(11, 24, 17, ...)` border family.
- Product hubs must expose an operational hierarchy rather than a generic action
  list. Keep primary and secondary products visually distinct while preserving
  every typed route and deterministic local-only behavior.
- Message category controls must remain one equal-width row from 320px through
  448px. Use a button group with `aria-pressed` unless a complete keyboard tab
  model is implemented, and distinguish unread rows structurally as well as by
  color.
- Root navigation keyboard focus belongs to the icon target, not the full grid
  cell. Keep the layer order explicit: content below the shaped navigation,
  navigation below route transitions, and transitions below sticky headers.
- Loan product comparison remains two columns at normal phone widths and
  collapses to one column at 340px and below. Both layouts must keep 44px touch
  targets and avoid horizontal page overflow.
- Light-theme seconds trading uses a bright market board with dark text; retain
  the separate dark-theme instrument panel rather than sharing one dark board
  across both themes.
- Keep bright `--signal-green` for positive fills, charts, and signal surfaces.
  Positive body text uses the contrast-safe `--positive` role and must reach
  WCAG AA against `--surface`; do not use the bright fill token as small text.
- Trading percentage shortcuts must derive quantity or margin from the
  authenticated user's real spot or margin available balance. Never use a
  fixed demo budget, and never fall back to a different margin product when the
  selected pair has no exact product match.
- Market chart inputs may contain epoch seconds or milliseconds. Normalize to
  UTC seconds before deduplication and sorting, ignore invalid rows, and skip
  zero-sized `ResizeObserver` measurements without destroying the empty canvas.

Run from `mobile/sites-prototype/` after shared surface changes:

```bash
npm run lint
npm test
git diff --check
```

**Language**: All code-spec documentation is written in English. User-facing mobile copy is defined in locale resources.

## Production Prototype Snapshot Contract

When the approved Sites prototype is promoted into `mobile/src/`, production
must own a tracked, self-contained snapshot of every visual build input:

- Keep the exact approved base CSS in `src/styles/prototype-base.css`. Layer
  production-only API, accessibility, and runtime corrections after it in
  `src/styles/prototype-parity.css`; do not import from the ignored
  `mobile/sites-prototype/` workspace.
- Copy required Geist fonts and HIPPO brand/stage images into tracked
  `src/assets/` paths and resolve every CSS `url(...)` through those paths.
  Tests must fail when a CSS asset reference does not exist or a production
  source/test file references `sites-prototype`.
- The snapshot controls geometry, typography, color, and static copy. Dynamic
  market, account, security, loan, message, and order values still come from
  the existing API/store contracts.
- Loading and failed API states must preserve the prototype's dimensions while
  showing skeletons, `--`, disabled controls, or explicit errors. Never fill
  missing production data with prototype demo values.
- Trading percentage controls accept display values `25`, `50`, `75`, and
  `100`, but calculations must normalize them to `0.25`, `0.5`, `0.75`, and
  `1`. Contract input/submission remains margin amount; displayed notional is
  `marginAmount * leverage`.

Required assertions:

```text
mobile/src and mobile/tests contain no sites-prototype dependency
every CSS font/image URL resolves to a tracked production asset
spot percentage derives from the exact base/quote wallet
contract percentage derives from the exact matched margin wallet
failed API loads expose no fabricated balances, scores, counts, or trends
```
