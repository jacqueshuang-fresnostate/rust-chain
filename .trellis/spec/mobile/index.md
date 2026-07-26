# Mobile Development Guidelines

> Executable conventions for the Vue 3 + Vite + Tauri mobile client in `mobile/`.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Navigation and Localization](./navigation-and-localization.md) | Router history, trade context, safe back behavior, and `vue-i18n` contracts | Active |

## Quality Check

Run from `mobile/` after navigation, localization, or shared UI changes:

```bash
npm run type-check
npm test
npm run build
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

Run from `mobile/sites-prototype/` after shared surface changes:

```bash
npm run lint
npm test
git diff --check
```

**Language**: All code-spec documentation is written in English. User-facing mobile copy is defined in locale resources.
