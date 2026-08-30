# Pencil 07c 秒合约交易对选择取证

## Selected frames

- Light `vONcc`: `07c / 秒合约 · 交易对选择 · 浅色主题`
- Dark `kLXCs`: `07c / 秒合约 · 交易对选择 · 深色主题`
- Both frames: 390×920, no clipped/collapsed nodes in Pencil bounds inspection.

## Geometry

- Full overlay: `0,0,390,920`; light `#00000099`, dark `#000000B8`.
- Pair sheet: `0,80,390,840`; top radius 24; padding `[18,20,16,20]`; gap 14.
- Sheet head: `20,18,350,34`; title 20/700 Geist; visible close 34 with Lucide X 18.
- Search: `20,66,350,46`; radius 12; horizontal padding 14; gap 10; Search icon 18; placeholder 13/450.
- List: `20,126,350,208`; 8px gap; each row 350×64, radius 12, horizontal padding 14, gap 12.
- Row logo: 30×30; pair 15/700 Geist Mono; price 14/650 Geist Mono; selected Check 17.
- Note: `20,348,350,14`; 11/450 Geist.

## Light palette

- Sheet/text: `#FFFFFF` / `#111714`
- Sheet border/shadow: `#DDE7E1` / `#00000022`
- Close surface/border/icon: `#E8F0EC` / `#CBD8D1` / `#25372D`
- Search surface/border/muted: `#F4F7F5` / `#CCD5D0` / `#68736D`
- Selected row/signal/check: `#D9F9EB` / `#43EFA9` / `#087B52`
- Default row/border: `#FFFFFF` / `#CCD5D0`

## Dark palette

- Sheet/text: `#0B0F0D` / `#F2F7F4`
- Sheet border/shadow: `#2C3A32` / `#00000044`
- Close surface/border/icon: `#1A251F` / `#2B3B32` / `#B4C1BA`
- Search surface/border/muted: `#151F1A` / `#29342E` / `#95A19A`
- Selected row/signal/check: `#103326` / `#43EFA9` / `#61F1B6`
- Default row/border: `#0C100E` / `#29342E`

## Production mapping

- Replace the invisible native Header select with a dialog trigger.
- Render only `SecondsProduct[]` from the API; no static sample rows.
- Price is `latestPriceForSymbol(product.symbol)` so the existing product-wide ticker subscription remains authoritative, with `marketStore` snapshot fallback.
- Logo is the backend market asset image through `AssetMark`, preferring `baseIconUrl` and then `iconUrl`.
- Selecting a row calls the existing `selectProduct(product)` and closes the dialog; it does not mutate `orders`.
