# Pencil PWA 安装弹窗真值

## Source

- Design file: `mobile/pencil/hippo-mobile-uiux.pen`（仅通过 Pencil CLI/MCP 读取）。
- Light frame: `NROQD` — `01b / PWA 安装提示 · 浅色主题`.
- Dark frame: `Tcgl6` — `01b / PWA 安装提示 · 深色主题`.
- Exported references: `/private/tmp/pwa-pencil/NROQD.png`, `/private/tmp/pwa-pencil/Tcgl6.png`.
- Read date: 2026-09-03.

## Baseline geometry

- Canvas: 390×1144.
- Overlay: x=0, y=0, w=390, h=1144; light node `R1dlXl`
  is `#07110C80`, dark node `AgFcl` is `#000000B8`.
- Sheet: x=0, y=604, w=390, h=540; vertical gap 14; padding `[12,20,22,20]`; top corners 26.
- Sheet effect in both themes: outer shadow `#00000024`, offset `0/-8`, blur `28`.
- Grabber: x=174, y=6, w=42, h=4, radius 2.
- Header: x=20, y=12, w=350, h=82, gap 14, top padding 10.
- App icon: 64×64 at x=0/y=14 inside header, radius 18; logo image 50×32 at x=7/y=16.
- Header copy: x=78/y=18.5, w=222, h=55, vertical gap 4.
- Close face: 36×36 at x=314/y=28, radius 18, with a 19×19 Lucide `x`;
  keep a 44×44 touch target without changing the visible face.
- Description: x=20/y=108, w=350, h=44, 14px/500, line-height 1.55.
- Benefit panel: x=20/y=166, w=350, h=146, vertical, padding `[8,14]`, radius 16.
- iPhone hint: x=20/y=326, w=350, h=42, horizontal padding 12, gap 8, radius 12.
- Primary CTA: x=20/y=382, w=350, h=54, radius 16, gap 8, with a
  19×19 Lucide `download`; outer shadow `#18D38D2E`, offset `0/6`, blur `16`.
- Later CTA: x=20/y=450, w=350, h=38.

## Typography and copy

- Title: `安装 Hippo App`, Noto Sans SC, 22px, 700, 32px box.
- Pencil defines no `letterSpacing` on the title or other modal copy; production
  must therefore resolve to zero tracking rather than adding negative tracking.
- Subtitle: `添加到主屏幕`, 13px, 500, mint.
- Description: `无需应用商店，获得更快的启动速度、全屏体验和及时通知。`
- Benefits:
  - `快速启动` / `从主屏幕一键进入`
  - `沉浸体验` / `全屏浏览，操作更专注`
  - `及时通知` / `重要行情与订单状态不错过`
- Hint: `iPhone 用户可通过分享菜单添加到主屏幕`.
- Primary: `立即安装`; secondary: `稍后提醒`.

## Benefit rows

- Each row: 322×43, gap 12.
- Icon plate: 30×30 at y=6.5, radius 9; icon 16×16 inset 7.
- Copy: x=42/y=2.5, h=38, gap 1.
- Heading: 14px/650, h=20; description: 12px/500, h=17.
- Lucide icons: `zap`, `maximize`, `bell-ring`; hint `info`; CTA `download`; close `x`.

## Light palette

- Sheet `#FFFFFF`; grabber `#CDD7D1`.
- App icon fill/stroke `#E3FAEF` / `#BCEBD6`.
- Title `#102018`; secondary text `#64736B`; mint `#18D38D`.
- Close face `#EFF8F3`.
- Benefit panel `#EFF8F3`; icon plate `#D8F7E9`.
- Hint fill/stroke `#F7FAF8` / `#DCE9E2`.
- Primary fill/text `#18D38D` / `#082A1D`.

## Dark palette

- Sheet `#101A15`; grabber `#526159`.
- App icon fill/stroke `#183D2F` / `#2E7057`.
- Title `#F4FAF7`; secondary text `#9BA8A1`; mint `#18D38D`.
- Close face `#18261F`.
- Benefit panel `#18261F`; icon plate `#214235`.
- Hint fill/stroke `#151F1A` / `#293B32`.
- Primary fill/text `#18D38D` / `#082A1D`.

## Repository mapping

- Reuse `hippo-logo-landscape.png` for the 50×32 logo image.
- Reuse `useModalDialog` for body lock, Escape, Tab loop, initial focus and focus restoration.
- Keep the current safe-route and eligibility logic; render only the install branch as a bottom modal.
- Keep other PWA states in the current non-blocking status island to avoid changing unrelated behavior.
