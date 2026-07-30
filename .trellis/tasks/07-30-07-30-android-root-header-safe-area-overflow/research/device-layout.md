# Android RootHeader Layout Audit

Device: Huawei TAS-AL00
WebView URL: `http://tauri.localhost/#/`

## Before Fix

```text
viewport: 360x780 CSS px
devicePixelRatio: 3
safe-area-inset-top: 35px
document scroll width: 360px
RootHeader rect: top 0px, bottom 64px, height 64px
RootHeader client height: 63px
RootHeader scroll height: 71px
RootHeader padding: 35px 16px 0
brand rect: top 27px, bottom 71px, height 44px
actions rect: top 27px, bottom 71px, height 44px
```

The failure is vertical, not horizontal. The fixed 64px border-box includes a
35px safe-area padding, leaving insufficient content height for the 44px
controls. The content is centered in the remaining area and overflows the
header bottom by 7px.

## Intended Geometry

Use a 56px content rail plus:

```css
max(8px, env(safe-area-inset-top, 0px))
```

as the top inset. This retains the approved 64px total height when the safe
area is zero, while a 35px Android inset produces a 91px total height without
shrinking or displacing the 44px controls outside the header.

## After Fix

```text
viewport: 360x780 CSS px
devicePixelRatio: 3
safe-area-inset-top: 35px
document scroll width: 360px
RootHeader rect: top 0px, bottom 91px, height 91px
RootHeader client height: 90px
RootHeader scroll height: 90px
RootHeader padding: 35px 16px 0
brand rect: top 40.5px, bottom 84.5px, height 44px
actions rect: top 40.5px, bottom 84.5px, height 44px
fitsHeader: true
noInternalOverflow: true
```

The app was rebuilt, installed, and launched on the same TAS-AL00. Android
reported `MainActivity` as resumed, visible, fully drawn, and focused.
