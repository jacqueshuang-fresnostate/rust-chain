# Selected Pencil truth: mobile new-coin flows

## Source of truth

- Canvas: `mobile/pencil/hippo-mobile-uiux.pen` (read only through Pencil MCP).
- Reference size: `390 x 920`.
- Selected frames:
  - `oOJ0q` / `ZTtvY`: New Coin Zone, light / dark.
  - `nFwYy` / `B6Qh9J`: New Coin Detail, light / dark.
  - `A9It6g` / `h4gfd`: New Coin Records, light / dark.
  - `XG67j` / `E2qzxN`: Trading Opportunities, light / dark.
- The 28px Pencil status bar represents native operating-system chrome and is not duplicated in the web document. Production measurements below are relative to the business header after removing that native row.

## Shared visual language

- Font: `Noto Sans SC` for Chinese copy, `Geist` or `Inter` for Latin headings, `Geist Mono` for dense financial/record values.
- Icons: Lucide only. Header Back uses `chevron-left`; Records uses `receipt-text`; Share uses `share-2`; Records filter uses `sliders-horizontal`.
- Light root canvases: Zone/Opportunity `#FFFFFF`; Detail `#F6F8F6`; Records `#F6F7F4`.
- Dark root canvases: Zone/Opportunity/Detail `#070908`; Records `#060A08`.
- Light primary text is around `#101412` / `#102018`; muted around `#65706A`, `#69766F`, `#8B938F`, or `#919A95` according to the frame.
- Dark primary text is `#F3F6F4` / `#F4FAF7`; muted is `#76817B`, `#77827C`, `#8C9690`, `#8F9B94`, or `#A6B0AA` according to hierarchy.
- Signal mint is `#19D795` / `#19C98D`; dark readable mint is `#67EDBC` / `#70C9A5`.
- All visible controls must retain at least a 44px pointer target even when Pencil draws a 22–40px face.
- Backend/project/ticker/wallet values remain authoritative. Pencil sample names and values are geometry references, never runtime fallbacks.

## New Coin Zone (`oOJ0q` / `ZTtvY`)

### Geometry

- Business header: 54px, 16px horizontal inset. Back face 26px; centered title `21/700/30`; Records icon face 23px.
- Banner section: 148px. Exact banner is `358 x 132`, inset `8px 16px`, radius 14.
- Primary tabs: 50px high, 16px left inset, 36px gap. Active label `17/700/25`, 72px track, 3px underline; inactive `17/550/25`.
- New-coin secondary filters: 36px high, `3px 16px`, 8px gap. Selected face 30px high/radius 9; labels `12px`.
- Content: starts after the filters, `8px 16px 18px`, 12px gap.
- Section heading: 36px; `18/700/26`; 20px ChevronRight face.
- Project card: `358 x 300`, radius 22, 1px border, 14px padding, 8px vertical gap, no active shadow.
  - Identity row `330 x 44`; logo 40px circle; symbol `18/750/26`; subtitle `10/500/14`; status 66x26 radius 9.
  - Main stats row 48px. Left and right values `21/750/30`, labels `11/500/16`.
  - Two 26px key/value rows.
  - Countdown 40px radius 10 with 10px horizontal padding.
  - Primary action `330 x 42`, radius 13, centered `15/750/22`.
- Exact banner bitmap is the selected generated image `mobile/pencil/images/generated-1788508119274.png`; production must own a tracked copy under `mobile/src/assets/`.

### Content and behavior

- Primary tabs are New Coin Activities and Trading Opportunities.
- Project lifecycle filters are All, Preheat, Subscription, Pending Listing, and Listed.
- Cards navigate to the typed project-detail route. The header Records action navigates to New Coin Records.
- Real project Logo/name, supply/progress, issue/quote data, unlock/listing timing, lifecycle and availability replace sample CP data while preserving the exact geometry.

### Theme

- Light: card `#FFFFFF`, border `#E7ECE9`, countdown `#F5F6F5`, selected filter `#F3F5F3`, dark action `#101412` with white text.
- Dark: root `#070908`, title chrome `#0F1110`, card `#111512`, border `#29322D`, countdown `#191E1B`, selected filter `#1A201C`, light action `#F1F5F2` with `#101412` text.

## Trading Opportunities (`XG67j` / `E2qzxN`)

### Geometry

- Reuses the same 54px header, 148px banner and 50px primary tabs.
- Content starts directly below primary tabs: `12px 16px 18px`, 10px gap.
- Filter rail: `358 x 36`, four equal 85px faces with 6px gaps; radius 12; label `11px`.
- Opportunity card: `358 x 140`, radius 18, 1px border, 12px padding, 8px gap.
  - Identity row 36px; logo 36px circle; pair `14/700/17`; project name `9/500/11`; launch badge 22px high/radius 8.
  - Three equal market metrics in a 36px row, 8px gaps; labels `9px`, values `12/700/15`.
  - Footer 24px; market descriptor `9px`; action face 64x24/radius 12 with `arrow-up-right`.

### Content and behavior

- Filters are All, Upcoming, Listed Today, and Hot Gains.
- Source only real new-coin projects that have an authoritative post-listing pair and real market ticker. Missing price/volume renders `--`; no sample DEBIT/TMX/DGAI rows.
- Action navigates to the real Spot Trade route for that pair. Switching primary tabs does not add browser history or refetch already-fresh reference data.

### Theme

- Light cards `#FFFFFF`, border `#E5EAE7`, inactive filter `#F0F3F1`, dark active/action `#101412`.
- Dark cards `#111513`, border `#28312C`, inactive filter `#151A18`, light active/action `#F3F6F4` with dark text.

## New Coin Detail (`nFwYy` / `B6Qh9J`)

### Geometry

- Business header: 56px, 16px inset. 26px Back, centered `21/700/30` title, 22px Share.
- Project visual: 210px high with 16px padding and 8px gap; light gradient `#E9F7F0 -> #DDF2E8`, dark `#101713 -> #17382C`.
  - Identity/status 56px; backend Logo 52px with 18px radius; symbol `22/800/27`; name `10/500/12`; state 69x26/radius 13.
  - Description `10/500`, 14px line.
  - Two data plates, each 175x48/radius 14, icon 17px; labels 9px; values `12/750`.
  - Progress line 22px; track 8px; percentage 10px.
- Stage section: 112px, 14px 16px padding. Heading 22px; four equal 85x52 stage cells with 6px gaps and radius 14; icon plates 22px; labels 10px.
- Rules section: 104px, `10px 16px`. Rule card 358x54/radius 16, three equal columns with 10px inset.
- Subscription panel: 328px, top corners radius 26, 16px padding, 11px gaps.
  - Heading 26px, balances row 24px.
  - Amount field 358x56/radius 17 with floating 9px label and `20/750` amount.
  - Four equal percentage controls, 32px high/radius 11, 8px gap.
  - Estimate row 22px.
  - Primary action 358x52/radius 26 with 20px left text and 38px mint arrow circle.
  - Risk hint 9px.

### Content and behavior

- Uses the project’s authoritative quote asset, wallet balance, issue price, supply counters, lifecycle, listing/unlock configuration and backend Logo/name.
- Percentage controls are 25/50/75/All. Only the selected percentage gets the mint-soft state.
- Subscription and post-listing purchase payload semantics remain unchanged and exact-decimal. The existing review dialog, focus trap, Escape/backdrop dismissal and mutation safety stay operational even though the page composition changes.
- Loading/error/guest/unavailable states preserve the selected tracks and never insert the Pencil sample data.

## New Coin Records (`A9It6g` / `h4gfd`)

### Geometry

- Business header: 58px, 16px inset. Back face 26px, title `22/700/32`, filter face 34px circle with 17px `sliders-horizontal`.
- Status filters: 56px high, `8px 16px`, four equal 85x40 pills, 6px gaps, radius 20, `12px` labels.
- List: `10px 16px 20px`, 14px gaps.
- Record card: `358 x 168`, radius 18, 1px border, subtle `0 4px 14px` shadow, clipped 4px left status rail.
  - Inner content starts after the rail, 14px padding, 10px gaps.
  - Header 326x40; Logo 40px circle; title `15/750/22`; time `9/500/13`; state 57x24/radius 12.
  - Main metrics 326x54 with bottom 1px divider and 9px bottom padding; 189px left track, 1px x 34px separator, 112px right track.
  - Primary metric uses `Geist Mono 15/750/19`; amount uses `Geist Mono 13/700/17`.
  - Footer 326x26; record number `Geist Mono 9px`; contextual operation and 24px arrow circle.

### Content and behavior

- Visible filters are All, In Progress, Pending Settlement, and Completed. They filter one chronological presentation assembled from all four authoritative APIs: subscriptions, distributions, purchases and unlocks.
- Preserve fee payment and release operations for actionable unlock records; fit the action into the selected footer without changing the card’s core track.
- Cards use backend project/asset Logo and name. Missing image falls back through `AssetMark`; missing project association shows the exact source identifier/localized fallback.
- The header filter button must expose a truthful action (for example opening a typed filter sheet) or remain a non-fabricated control; it must not silently do nothing.

## Responsive, accessibility, and verification contract

- `390px` runtime geometry must match the tracks above after excluding native status chrome.
- `320px` and `448px` must have zero document horizontal overflow; fixed Pencil tracks become fluid while preserving insets/gaps and 44px hit targets.
- Dark/light theme changes only palette, not geometry.
- Visible static copy comes from symmetric `zh-CN` and `en` locale keys.
- API load, cached-data-with-error, empty, guest, disabled and submitting branches remain distinguishable.
- Validate with focused source/behavior tests, Mobile type checks, complete Mobile tests, production PWA/Tauri builds, and Ego/Pencil-browser screenshots at 390px in both themes plus 320/448 overflow checks.
