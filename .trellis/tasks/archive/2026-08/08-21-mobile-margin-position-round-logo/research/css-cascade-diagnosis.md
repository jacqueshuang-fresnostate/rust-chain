# Margin position AssetMark cascade diagnosis

## Runtime observation

Ego Browser task space 34 opened:

`http://127.0.0.1:1611/#/trade/BTC_USDT?mode=contract`

The live header `AssetMark` loaded the backend 96x96 BTC WebP and computed as:

- root: 28x28, `border-radius: 50%`, `overflow: hidden`, zero padding;
- image: 28x28, `border-radius: 50%`, `object-fit: cover`;
- image pixels have transparent corners and a circular BTC face.

## Reproduction

The same rendered `AssetMark` root was cloned into a `.contract-position-identity` fixture while retaining Vue's parent scope attribute. Runtime computed style changed to:

- root: 24x24, `border-radius: 3px`, `padding: 0 5px`;
- image: `border-radius: 3px` through `inherit`.

The compiled winning rule is:

```css
.contract-position-identity span[data-v-d392f1f9] {
  background: var(--contract-surface-soft);
  border-radius: 3px;
  color: var(--contract-muted);
  font-size: 8px;
  line-height: 16px;
  padding: 0 5px;
}
```

Vue forwards the parent scope attribute onto the child component root, and `AssetMark` uses a root `<span>`. The broad descendant selector therefore treats the component root as a position badge and outranks the shared `.asset-mark` rule.

## Decision

Target only the explicit position badge elements rather than all descendant spans. Keep the shared `AssetMark` contract unchanged because its standalone runtime behavior is already correct. Add a regression assertion that rejects the broad selector and verifies the badge-only selector.

## Post-fix runtime verification

Ego Browser rechecked the local contract position fixture at a 390x844
viewport in both light and dark themes:

- the `AssetMark` root and `img` are both 24x24 with `border-radius: 50%`;
- the root has zero padding, `overflow: hidden`, no border, and no shadow;
- document horizontal overflow remains zero.

The final scoped selector is limited to the three explicit
`.contract-position-badge` elements, so none of those badge declarations match
the sibling `AssetMark` root.
