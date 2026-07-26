# Research: Award-Level Mobile Web Art Direction and Interaction Patterns

- Query: Research contemporary award-level mobile web art direction and interaction patterns suitable for a high-end crypto exchange prototype; identify 3–5 comparable Awwwards/FWA/CSSDA-style patterns or awarded experiences and map them to the existing React/Vinext Sites prototype without weakening dense transactional usability.
- Scope: mixed
- Date: 2026-07-26

## Findings

### Executive recommendation

Use a **two-speed art direction system**:

1. **Expressive brand layer** — Home, Markets intro, asset hero, profile identity, product discovery, and the desktop presentation stage may use monumental type, one signature visual motif, restrained texture, and choreographed reveals.
2. **Protected transaction layer** — Spot, Contract, order forms, funding, KYC, authentication, records, and other secondary workflows keep a stable grid, native scrolling, semantic HTML, instant controls, and minimal feedback motion.

The award-level quality should come from a coherent type system, a single recurring “signal/coin/orbit” motif, disciplined transition choreography, and unusually polished state feedback—not from applying WebGL, parallax, blur, or scroll hijacking to every route.

The most suitable external references are Jeton for a coherent fintech motif, Igloo Inc for material atmosphere and transition staging, DeSo for editorial typography and color worlds, and Contra Project Calculator for tactile functional controls. None should be copied wholesale: all four are primarily storytelling or campaign experiences, while HIPPO is a dense exchange workspace.

### Files found

| File | One-line description |
| --- | --- |
| `mobile/sites-prototype/app/page.tsx` | Main React client prototype containing six root exchange views, Canvas/SVG graphics, route switching, transactional forms, feedback, and navigation. |
| `mobile/sites-prototype/app/globals.css` | Global visual system, motion, responsive behavior, dense exchange overrides, secondary-page styles, and reduced-motion rules. |
| `mobile/sites-prototype/app/layout.tsx` | Metadata and current Geist/Geist Mono font setup. |
| `mobile/sites-prototype/app/prototype-routes.ts` | Typed root and secondary route inventory, titles, protected routes, and fallback behavior. |
| `mobile/sites-prototype/app/secondary-pages.tsx` | Thirty-nine secondary product, funding, account, authentication, and compliance workflows with validation and ARIA states. |
| `mobile/sites-prototype/tests/rendered-html.test.mjs` | SSR, navigation, reduced-motion, 390px, safe-area, reachability, transactional, and no-inert-control source contracts. |
| `mobile/sites-prototype/package.json` | React 19.2.6, Next 16.2.6, Vinext 0.0.50, Vite 8.0.13; no dedicated animation or WebGL runtime dependency. |
| `.trellis/spec/mobile/navigation-and-localization.md` | Mobile contract for navigation, localization, safe back behavior, 390px overflow, bottom-nav occlusion, and reachable controls. |

### Existing code patterns and constraints

#### Platform and information density

- The prototype is React 19.2.6 on Next 16.2.6 through Vinext 0.0.50 and Vite 8.0.13. There is no GSAP, Motion, Three.js, or Rive dependency, so CSS/SVG/native browser primitives are the lowest-risk direction (`mobile/sites-prototype/package.json:16`).
- Six root routes—Home, Markets, Spot, Contract, Assets, and Profile—are persistent first-class columns (`mobile/sites-prototype/app/page.tsx:151`; `mobile/sites-prototype/app/prototype-routes.ts:1`).
- Thirty-nine typed secondary routes cover market detail, orders, deposits, withdrawals, transfers, KYC, authentication, lending, earning, new coins, prediction, and account settings (`mobile/sites-prototype/app/prototype-routes.ts:12`; `mobile/sites-prototype/tests/rendered-html.test.mjs:93`).
- Home already prioritizes portfolio value, funding actions, eight product shortcuts, news, and five market rows (`mobile/sites-prototype/app/page.tsx:393`; `mobile/sites-prototype/app/page.tsx:444`; `mobile/sites-prototype/app/page.tsx:492`; `mobile/sites-prototype/app/page.tsx:503`; `mobile/sites-prototype/app/page.tsx:524`).
- Spot and Contract share a dense but mode-aware transaction surface: symbol/quote, chart timeframe controls, contract settings, buy/sell state, order type, amount, range control, balance, risk copy, and submission (`mobile/sites-prototype/app/page.tsx:728`; `mobile/sites-prototype/app/page.tsx:793`; `mobile/sites-prototype/app/page.tsx:839`; `mobile/sites-prototype/app/page.tsx:854`; `mobile/sites-prototype/app/page.tsx:884`; `mobile/sites-prototype/app/page.tsx:912`; `mobile/sites-prototype/app/page.tsx:935`).

#### Existing art direction

- The base palette already has a useful exchange semantic system: near-black surfaces, high-value light text, muted neutrals, green positive/buy, coral negative/sell, cyan information/contract, and yellow warning/favorite (`mobile/sites-prototype/app/globals.css:3`).
- The “Version 2: exchange-first mobile workspace” override deliberately tightens the earlier editorial concept into a more conventional exchange interface (`mobile/sites-prototype/app/globals.css:2107`).
- The desktop frame already acts as a poster-like brand layer with oversized “MARKETS IN MOTION” typography and a separate 430px mobile canvas (`mobile/sites-prototype/app/page.tsx:1393`; `mobile/sites-prototype/app/globals.css:106`; `mobile/sites-prototype/app/globals.css:170`).
- The mobile Home is deliberately utilitarian: a search/utility row, large account number, compact performance metadata, SVG portfolio chart, funding CTAs, and tabular market rows (`mobile/sites-prototype/app/globals.css:2201`; `mobile/sites-prototype/app/globals.css:2233`; `mobile/sites-prototype/app/globals.css:2288`; `mobile/sites-prototype/app/globals.css:2348`; `mobile/sites-prototype/app/globals.css:2515`).
- Geist is configured only with its Latin subset, while Chinese falls through to system CJK faces (`mobile/sites-prototype/app/layout.tsx:1`; `mobile/sites-prototype/app/globals.css:31`). This is performant, but the display identity and the Chinese product UI currently do not share a purpose-built CJK typographic voice.
- Many dense labels use 7–9px text, including bottom-nav labels, table metadata, order types, helpers, and disclaimers (`mobile/sites-prototype/app/globals.css:922`; `mobile/sites-prototype/app/globals.css:1042`; `mobile/sites-prototype/app/globals.css:1253`; `mobile/sites-prototype/app/globals.css:1364`; `mobile/sites-prototype/app/globals.css:1928`). This creates a legibility ceiling that new visual effects would worsen.

#### Existing motion and graphics

- `SignalField` is a decorative 2D Canvas animation with a device-pixel-ratio cap of 2, `requestAnimationFrame`, cleanup, and a reduced-motion static frame (`mobile/sites-prototype/app/page.tsx:160`; `mobile/sites-prototype/app/page.tsx:169`; `mobile/sites-prototype/app/page.tsx:175`; `mobile/sites-prototype/app/page.tsx:219`; `mobile/sites-prototype/app/page.tsx:226`).
- `TradeChart` is a separately redrawn 2D Canvas chart, while market-row sparklines and the Home portfolio chart use SVG (`mobile/sites-prototype/app/page.tsx:235`; `mobile/sites-prototype/app/page.tsx:302`; `mobile/sites-prototype/app/page.tsx:470`).
- Every root or secondary view remount currently receives the same 300ms–480ms opacity/vertical movement/**blur** entrance (`mobile/sites-prototype/app/page.tsx:1416`; `mobile/sites-prototype/app/globals.css:295`; `mobile/sites-prototype/app/globals.css:304`; `mobile/sites-prototype/app/globals.css:2192`). The blur adds paint cost and makes transaction-to-transaction navigation feel less precise.
- Infinite CSS motion already exists in signal bars and the asset orbit (`mobile/sites-prototype/app/globals.css:721`; `mobile/sites-prototype/app/globals.css:1452`). A global reduced-motion rule collapses animations and transitions to 0.01ms (`mobile/sites-prototype/app/globals.css:2856`).
- Sticky top and bottom chrome uses `backdrop-filter` blur (`mobile/sites-prototype/app/globals.css:209`; `mobile/sites-prototype/app/globals.css:1892`). This is visually polished but can be costly when a large chart or Canvas animates underneath it.
- The six-item bottom navigation is safe-area aware and preserves 56–58px row height at 390px, although the visible labels shrink to 7.5px (`mobile/sites-prototype/app/globals.css:1892`; `mobile/sites-prototype/app/globals.css:2797`).

#### Existing accessibility and product contracts

- Keyboard focus is visibly styled with a cyan outline (`mobile/sites-prototype/app/globals.css:53`).
- Root controls have labels/current-page state, tabs use tab semantics, errors use alerts, and toast feedback uses `role="status"` plus `aria-live="polite"` (`mobile/sites-prototype/app/page.tsx:533`; `mobile/sites-prototype/app/page.tsx:1519`; `mobile/sites-prototype/app/page.tsx:1534`; `mobile/sites-prototype/app/secondary-pages.tsx:356`; `mobile/sites-prototype/app/secondary-pages.tsx:595`).
- The Canvas trade chart has an `aria-label`, but Canvas pixels themselves remain unavailable to assistive technology; the current chart lacks an adjacent structured price summary or fallback content (`mobile/sites-prototype/app/page.tsx:299`).
- Tests explicitly preserve reduced motion, 820px/390px responsive rules, six-column navigation, and bottom safe-area handling (`mobile/sites-prototype/tests/rendered-html.test.mjs:53`).
- The mobile spec requires 390px checks for horizontal overflow, bottom-nav occlusion, and reachable language controls (`.trellis/spec/mobile/navigation-and-localization.md:125`).

### Comparable award-level patterns

#### 1. Jeton — one tactile motif across brand, product, and motion

**Award evidence and pattern**

- Jeton’s 2025 award record includes Awwwards Developer Award, Honors, and Site of the Day; CSSDA Site of the Day; and FWA Site of the Day.
- The identity builds almost everything from one matte disc/coin: custom wordmark, 3D brand universe, feature loops, app imagery, and transaction metaphors.
- Its orange-and-pastel color system avoids default fintech black, and its narrative uses strong keywords, clear visuals, scroll-linked Rive work, and purpose-specific 3D loops. The system was designed across the marketing site, app, and web app and prepared for 19 languages.

**Why it transfers**

- HIPPO already has compatible primitives: round coin badges, the asset orbit, chart dots, market temperature, and “Signals in Motion.”
- The transferable lesson is not “add 3D coins everywhere.” It is **choose one formal grammar and repeat it consistently**.

**Mapping to HIPPO**

- Make the signature object a **signal disc**: concentric ring, clipped pulse line, or offset arc. Use it in the brand mark, coin badges, active navigation, chart current-point marker, asset hero, loading states, and success confirmation.
- Keep the object dimensional only in Home/Assets/Profile. In Markets and Trade it collapses into a flat 1px/2px ring or status dot.
- Implement with CSS borders, radial/conic gradients, and lightweight SVG masks. A pre-rendered AVIF/WebP loop can cover one hero if more dimensionality is needed. No runtime 3D is required.
- Preserve green/coral as semantic buy/sell colors. A warmer brand accent may frame discovery surfaces, but it must never replace trading semantics.

**Tradeoff**

- A single motif creates coherence cheaply; custom 3D loops create an asset pipeline, localization/cropping burden, and LCP risk. Use dimensional assets as optional decoration after core UI paint.

Sources:

- [Jeton.com — 2025 CCP Festival case and award record](https://www.oneclub.org/awards/ccp/-award/59206/jetoncom/)
- [Jeton fintech website pattern analysis](https://dokle.design/websites/jeton-fintech-website-design-bottom-nav)

#### 2. Igloo Inc — material atmosphere and scene transitions, isolated from utility

**Award evidence and pattern**

- Igloo Inc received Awwwards Site of the Year and Developer Site of the Year for 2024.
- Its experience uses a frozen material world, chromatic aberration, displacement, frost transitions, procedural ice, real-time intro animation, and interactive particles.
- The creators first mapped the user journey in grey mockups and low-detail previs, then iterated in-browser while measuring low-end-device performance.
- Their full WebGL UI was a targeted response to shader glitches and text-scramble cost, not a generic recommendation for application interfaces.

**Why it transfers**

- HIPPO can benefit from the **material logic**—cold glass, fine grain, refracted signal lines, and a small number of memorable transitions—without adopting the all-WebGL implementation.
- The strongest lesson is production discipline: previsualize the movement and measure it on constrained devices before polishing the material.

**Mapping to HIPPO**

- Restrict “frost/signal glass” to the desktop stage, Home portfolio background, Assets hero, and optional page-intro bands.
- Build texture from one compressed monochrome noise tile or an inline SVG turbulence asset composited at 1–3% opacity, plus static gradients. Never place animated noise over prices, order inputs, KYC forms, or body copy.
- Replace the current whole-view blur entrance with an opacity/translate reveal and one brief accent-line sweep. Reserve any displacement-like dissolve for Home → product discovery, never Market → Trade or within an order flow.
- If a future experimental WebGL hero is approved, mount it only after the semantic UI is interactive, lazy-load it as a separate chunk, provide a static image fallback, and disable it for reduced motion, save-data, low memory, background tabs, and transaction routes.

**Tradeoff**

- Igloo’s visual distinctiveness depends on specialized technical-art tooling (Three.js, Svelte, GSAP, Houdini, Blender, custom exporters). Rebuilding that stack inside the existing React/Vinext transaction prototype would add large runtime and maintenance cost for little operational value.

Sources:

- [Awwwards — Igloo Inc case study](https://www.awwwards.com/igloo-inc-case-study.html)
- [Awwwards listing showing Igloo Inc as 2024 SOTY and Developer Award](https://www.awwwards.com/websites/%2307998E/?page=5)

#### 3. DeSo Foundation — oversized editorial type and color-coded worlds

**Award evidence and pattern**

- DeSo Foundation by Studio Freight won CSSDA Website of the Month in April 2023 and became a 2023 Website of the Year nominee. CSSDA recorded an 8.93 final judge score and classified it as animated and colorful.
- Contemporary design coverage highlights DeSo’s near-viewport-width headline, dark gradient field, white display type, mixed styles, and asymmetric magazine-like spacing.

**Why it transfers**

- Dense exchanges often flatten every level into 10–14px neutral UI. DeSo demonstrates that one oversized editorial statement can create a strong identity while the rest of the page remains structured.

**Mapping to HIPPO**

- Keep monumental type to one statement per root view: Home’s brand proposition, Markets’ “market pulse” headline, Assets’ total-value moment, or the desktop stage. Do not put oversized copy above the fold on Spot/Contract.
- Use `clamp()` for display sizes and a Chinese-aware line-height; keep display copy to two short lines. Avoid outlined text over moving textures on mobile.
- Create restrained “color climates”: green/cyan for market intelligence, off-white/graphite for utility, coral only for risk/sell, and one non-semantic brand gradient for discovery. The table and form surfaces remain neutral.
- Animate typographic masks or underlines, not glyph contents. A one-time `clip-path`/translate reveal is preferable to continuous variable-font axis animation.

**Tradeoff**

- Oversized type consumes scarce mobile viewport space and can push key balances or actions below the fold. It must be used as a sectional event, not as the default page title system.

Sources:

- [CSSDA — DeSo Foundation 2023 Website of the Year nominee](https://www.cssdesignawards.com/woty2023/sites/deso-foundation/)
- [CSSDA — DeSo Foundation Website of the Month](https://www.cssdesignawards.com/wotm/deso-foundation/43170/)
- [DeSo oversized typography analysis](https://www.tbhcreative.com/blog/2024-web-design-trends/)

#### 4. Contra Project Calculator — tactile functional controls as the spectacle

**Award evidence and pattern**

- Contra Project Calculator by Unseen Studio won CSSDA Site of the Day and Site of the Month, FWA Site of the Day, Awwwards Site of the Day and Product Honours, and CSS Winner Site of the Day.
- The calculator turns a real comparison task into a retro-tech device with depressing buttons, clicking switches, levers, interlocking parts, and a carefully iterated responsive form.

**Why it transfers**

- This is the closest reference to a transaction surface: the interaction itself, rather than a background scene, carries the personality.

**Mapping to HIPPO**

- Give buy/sell tabs, order-type selectors, percentage presets, leverage/margin buttons, quote/review steps, and submit states a controlled tactile response.
- Use 1–2px translation, contrast change, inset border, and a 120–180ms transform/opacity transition. Do not use spring overshoot on financial confirmation controls.
- Turn the order flow into a visibly staged instrument: input → estimate/risk summary → submit/processing → result. The existing 420ms simulated submit and live-region toast already supply the state skeleton (`mobile/sites-prototype/app/page.tsx:778`; `mobile/sites-prototype/app/page.tsx:1534`).
- Keep all controls as semantic buttons, inputs, and labels. Visual switches must not require dragging; every drag/range interaction needs tap presets, which the current percentage control already provides (`mobile/sites-prototype/app/page.tsx:935`).

**Tradeoff**

- Tactility improves comprehension when it confirms a state change. Excessive bevels, sounds, physics, or long travel turn serious finance into a game and slow expert users.

Source:

- [Unseen Studio — Contra Hiring Calculator case study and awards](https://unseen.co/projects/hiring-calculator/)

### Recommended HIPPO design system

#### Typography

Use a three-role system:

1. **Display/brand** — Geist or one locally hosted display face for short Latin/brand statements; system CJK or a carefully subsetted CJK face for Chinese. Use only on Home, Markets intro, Assets hero, and stage art.
2. **Interface** — Geist Sans plus the existing CJK system stack for controls and prose.
3. **Data** — Geist Mono with `font-variant-numeric: tabular-nums` for prices, balances, percentages, leverage, and timestamps.

Recommended mobile size floor:

- Root display: 30–44px depending on available content.
- Section heading: 18–24px.
- Primary values: 24–32px.
- Body/control labels: 12–14px.
- Metadata: 10–12px.
- Avoid 7–9px for actionable labels, risk text, table headers, and bottom navigation. Preserve density with shorter copy, line-height, grid alignment, and disclosure—not microscopic type.

Keep line-length and font loading stable. Do not add a large remote CJK font payload merely to make the prototype feel “premium”; local subsetted WOFF2 or system CJK is preferable.

#### Motion hierarchy

Define motion by purpose:

| Tier | Purpose | Duration | Allowed properties | Routes |
| --- | --- | --- | --- | --- |
| M0 | Instant data/control state | 0–100ms | color, border, no transition where immediacy matters | Trade, funding, auth |
| M1 | Press/focus/selection feedback | 120–180ms | transform, opacity, color | All interactive controls |
| M2 | Local panel/tab change | 180–260ms | opacity, translate 4–8px | Markets, orders, settings |
| M3 | Root/shared-element navigation | 240–360ms | opacity, translate, clip; optional View Transition | Root/market detail |
| M4 | Brand reveal | 450–650ms once | transform, opacity, mask/clip | Home, Assets, stage |

Rules:

- Never delay navigation or data paint until an animation completes.
- Never animate price changes with large spatial movement; use a short color flash or digit crossfade.
- Keep continuous motion to at most one ambient element in a viewport.
- Disable ambient motion while the user is editing an order or sensitive form.
- Replace the current blur-based whole-view entrance with transform/opacity; blur is paint-heavy and reduces text crispness.
- Consider the native View Transition API only as progressive enhancement. Feature-detect it, skip when reduced motion is requested, preserve focus and scroll position, and avoid cloning live regions into a transition.

#### Shared-element continuity

The highest-value “award” interaction is a restrained market-row → market-detail/trade transition:

- Shared elements: coin disc, symbol, current price, and trend color.
- Outgoing list remains visually stable; selected row expands or crossfades into the detail header.
- On back, restore the list position and focus to the originating market row.
- Fallback is immediate React state replacement with a 180–240ms content fade.

This gives the perceptual continuity of a showcase site while directly improving a transactional journey.

#### Texture and depth

Preferred stack, lightest first:

1. CSS gradients, 1px hairlines, local shadows, `color-mix()`.
2. Inline SVG masks, turbulence/noise, sparklines, and reusable signal-disc shapes.
3. 2D Canvas only for high-frequency decorative signals or chart drawing.
4. Pre-rendered AVIF/WebP loops for a single hero.
5. WebGL only in an isolated, lazy, optional brand scene after performance proof.

Texture rules:

- Apply grain to containers, not the entire document.
- Keep opacity below the point where it competes with 10–14px text.
- Do not animate noise.
- Avoid large continuously blurred/translucent layers over Canvas content.
- Use static opaque chrome on low-end or reduced-transparency modes if blur costs become visible.

#### Canvas/SVG strategy

- Keep market sparklines and portfolio lines in SVG: they scale cleanly, remain cheap, and do not require a render loop.
- Keep `SignalField` Canvas only if it is visible and decorative; pause it via Intersection Observer or `contentvisibilityautostatechange` when offscreen, and update `prefers-reduced-motion` if the media query changes after mount.
- Keep the chart Canvas semantic sibling text: current price, period, high/low, direction, and a concise trend description. Canvas `aria-label` alone does not expose drawn data.
- Use the `requestAnimationFrame` timestamp rather than incrementing a frame counter so animation speed does not vary on 60/120/144Hz screens.
- If chart interaction is added, keep focusable DOM controls and textual values outside Canvas.

#### Accessibility and performance guardrails

- Target WCAG 2.2 AA. Pointer targets should be at least 24×24 CSS px under the minimum criterion; prefer 44×44 for primary mobile actions and dense controls where layout permits.
- Do not encode buy/sell, gain/loss, error/success, or risk solely by color; preserve text, sign, label, icon, or pattern.
- Preserve the existing focus ring, ARIA tab states, error alerts, current-page state, and live-region feedback.
- Reduced motion should remove or replace panning, scaling, orbiting, displacement, and background loops—not merely shorten them to an imperceptible duration.
- Keep native vertical scroll. Do not add Lenis-style scroll smoothing or scroll-jacking to the exchange canvas.
- Use `content-visibility: auto` plus an accurate `contain-intrinsic-size` selectively on long independent secondary sections/lists, and verify focus/search behavior.
- Core Web Vitals acceptance targets at the 75th percentile: LCP ≤ 2.5s, INP ≤ 200ms, CLS ≤ 0.1. For the prototype, also record dropped frames and long tasks on a throttled mid/low mobile profile.
- Do not ship a new animation runtime before CSS/SVG/native View Transitions have been proven insufficient.

Primary technical references:

- [MDN — View Transition API](https://developer.mozilla.org/en-US/docs/Web/API/View_Transition_API)
- [MDN — prefers-reduced-motion](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/At-rules/%40media/prefers-reduced-motion)
- [MDN — Canvas drawing and accessible fallback content](https://developer.mozilla.org/en-US/docs/Learn_web_development/Extensions/Client-side_APIs/Drawing_graphics)
- [MDN — content-visibility](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/content-visibility)
- [web.dev — high-performance CSS animations](https://web.dev/articles/animations-guide)
- [web.dev — Core Web Vitals](https://web.dev/articles/vitals)
- [W3C — WCAG 2.2](https://www.w3.org/TR/wcag/)

### Route-by-route mapping

| Surface | Recommended expression | Motion ceiling | Protected utility |
| --- | --- | --- | --- |
| Desktop stage | Monumental typography, static editorial image, fine grain, signal-disc motif | One slow ambient reveal | Keep mobile canvas independent and readable |
| Home | Strong portfolio value, one branded signal field, one dimensional motif, selective type scale | M4 once, then M1/M2 | Funding CTAs, balances, market rows paint first |
| Markets | One editorial headline, color climate, shared row/detail continuity | M3 navigation; rows otherwise static | Search, tabs, prices, favorites stay instant |
| Spot/Contract | Tactile controls and clear state feedback only | M0–M2; no ambient loop | Stable chart/form grid, no texture over data |
| Assets | Signature orbit/disc, restrained material hero, allocation reveal | One M4 hero plus M1 actions | Deposit/withdraw/transfer actions remain direct |
| Profile/Product hub | Editorial identity and modular cards | M2/M3 | Security/auth states remain conventional |
| Secondary finance/KYC/auth | Minimal page transition, staged progress, status feedback | M1/M2 | Validation, focus, back behavior, records dominate |

### Recommended implementation order

1. **Typography and legibility** — remove 7–9px actionable text, add tabular numerals, define display/interface/data roles.
2. **Motion tokens and route tiers** — replace universal blur entrance, cap durations, stop ambient animation on transaction routes.
3. **One signature motif** — normalize the current coin/orbit/signal fragments into a reusable CSS/SVG disc language.
4. **Shared-element market transition** — progressive View Transition enhancement with focus/scroll restoration.
5. **Texture pass** — static container-scoped grain and gradients on brand surfaces only.
6. **Performance/a11y validation** — 390px overflow/nav checks, keyboard/screen-reader pass, reduced-motion pass, throttled mobile trace, Core Web Vitals.
7. **Optional hero experiment** — only after the above passes; compare CSS/SVG, pre-rendered loop, and lazy WebGL variants against the same performance budget.

### Related specs

- `.trellis/spec/mobile/index.md` — mobile quality commands and package context.
- `.trellis/spec/mobile/navigation-and-localization.md:11` — navigation and localization scope.
- `.trellis/spec/mobile/navigation-and-localization.md:28` — root navigation/history contracts.
- `.trellis/spec/mobile/navigation-and-localization.md:99` — required browser and build checks.
- `.trellis/spec/mobile/navigation-and-localization.md:125` — local visual preview independence and 390px validation.
- `.trellis/spec/guides/index.md` — pre-modification search and cross-layer/reuse prompts.

## Caveats / Not Found

- `python3 ./.trellis/scripts/task.py current --source` returned `Current task: (none)` and `Source: none`. The user supplied the exact task and research path, so this file was written there without guessing another location.
- This is art-direction and interaction research, not a live visual or performance audit. No code, screenshots, design assets, production deployment, dependency changes, or browser traces were created.
- Award case studies are predominantly marketing/storytelling sites. Their award status does not prove fitness for order entry, market monitoring, KYC, or funds transfer; all recommendations above deliberately protect transactional routes.
- Some external sites evolve or disappear after award judging. Findings use award archives and published case studies where possible rather than assuming the current live site is identical to the awarded version.
- Exact View Transition support should be rechecked at implementation time. The API must remain progressive enhancement with a non-animated fallback.
- The existing stylesheet contains an original visual system followed by a large “Version 2” override (`mobile/sites-prototype/app/globals.css:2107`). Before implementation, overlapping selectors should be inventoried so new tokens do not create a third cascade layer.
- The current Canvas animation checks reduced motion only when its effect mounts. A runtime preference change is not observed.
- `TradeChart` redraws on resize but has no textual data fallback beyond its label; no screen-reader or zoom audit was performed.
- The project’s progress-log rule was not applied because this researcher’s write scope and the user’s explicit instruction both permit changes only inside the specified task `research/` directory.
