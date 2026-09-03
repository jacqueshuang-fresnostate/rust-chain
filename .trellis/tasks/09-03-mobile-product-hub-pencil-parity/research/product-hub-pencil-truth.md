# 产品中心 Pencil 真值与生产差异

## Source

- Active Pencil document: `mobile/pencil/hippo-mobile-uiux.pen`。
- Current mobile selections: `Z0B0N6`（14 / 产品中心 · 浅色主题）、`zMsKE`（14 / 产品中心 · 深色主题）。
- Also selected: `d2FQe`（产品中心分类标题区），属于设计画布分类说明，不是手机页面内容。
- Exports: `/private/tmp/product-pencil/Z0B0N6.png`、`/private/tmp/product-pencil/zMsKE.png`。

## Geometry

Pencil 画板为 390×920，其中前 28px 是操作系统状态栏。生产 Web/Tauri 页面不绘制该区域，因此以下运行时坐标均为 Pencil y 减 28px：

- Header: 390×60，运行时 y=0；20px 水平设计内边距，40×40 左右透明轨道。
- Back: 22×22，中心 `(40,30)`；Title: 72×26，x=159/y=17；Ellipsis: 18×18，中心 `(350,30)`。
- Body: 390×222，运行时 y=60，padding `[8,20,20,20]`，vertical gap 18。
- Product list: x=20/y=68，350×128。
- Each row: 350×64；icon plate x=0/y=10，44×44；copy x=58/y=11.5，260×41；chevron x=332/y=23，18×18。
- Help: x=20/y=214，350×48；BookOpen x=0/y=16，16×16；label x=26/y=14.5，79×19；chevron x=334/y=16，16×16。

## Typography and colors

- Header title: Geist 18px/750, 26px resolved height.
- Product title: Geist 15px/700, 22px resolved height.
- Product description: Geist 11px/450, 16px resolved height, `#7A8B80` in both themes.
- Help label: Geist 13px/650, 19px resolved height.
- Light: page `#FFFFFF`, text `#111714`, plate `#FFFFFF`, plate border `#CCD5D0`.
- Dark: page `#000000`, text `#F2F7F4`, plate `#0C100E`, plate border `#29342E`.
- Both themes: row/help chevrons `#7A8B80`; all other icons use theme text.
- Pencil rows and Help have no fill, stroke, radius, shadow or divider.

## Existing production delta at 390×920

Ego browser baseline before implementation:

- Header/body/row/help outer coordinates are already correct.
- Ellipsis resolves to 20×20 instead of 18×18.
- Each 64px row and the 48px Help render an extra bottom border.
- Product title resolves to 14px/700/20px instead of 15px/700/22px.
- Product description resolves to weight 400 and non-Pencil theme muted colors.
- Help resolves to 12px/400 muted instead of 13px/650 text.
- Light icons use green `rgb(0,122,77)` instead of `#111714`; dark icons use mint `rgb(82,226,161)` instead of `#F2F7F4`.
- Dark icon plate is `#000000` with translucent border instead of `#0C100E/#29342E`; light border is translucent instead of `#CCD5D0`.

## Implementation boundary

- Keep the existing fixed three actions and typed navigation.
- Correct normal-state pixels in `ProductHubView.vue` and product-only theme variables in `pencil-selected-pages.css`.
- Preserve 44px semantic touch targets and focus-visible feedback without adding any normal-state decoration.

## Final runtime verification

- At 390×920 in both themes, Header is `y=0..60`, Body is `y=60, h=222`,
  rows are `x=20, y=68/132, w=350, h=64`, and Help is
  `x=20, y=214, w=350, h=48`.
- Light text/page/plate/border resolve to
  `#111714/#ffffff/#ffffff/#ccd5d0`; dark values resolve to
  `#f2f7f4/#000000/#0c100e/#29342e`. Subtitle and chevrons remain `#7a8b80`.
- Product title, subtitle, and Help resolve to `15/700/22`, `11/450/16`, and
  `13/650/19`. The Header action is `x=328, y=8, 44×44`; Ellipsis is
  `x=341, y=21, 18×18`.
- Runtime checks at 320px, 390px, and 448px report zero horizontal overflow.
  Normal rows and Help have no fill, border, radius, shadow, or divider.
