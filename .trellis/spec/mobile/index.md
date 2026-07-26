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

**Language**: All code-spec documentation is written in English. User-facing mobile copy is defined in locale resources.
