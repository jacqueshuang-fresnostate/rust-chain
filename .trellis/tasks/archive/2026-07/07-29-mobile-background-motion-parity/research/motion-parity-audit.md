# Motion Parity Audit

## Sources

- Public reference: `https://hippo-mobile-signal-2026.ikuboy.chatgpt.site/?version=16`
- React runtime: `mobile/sites-prototype/app/page.tsx`
- Approved CSS snapshot: `mobile/src/styles/prototype-base.css`
- Production shell: `mobile/src/App.vue`

## 390x844 Runtime Findings

### Public v16

- `.mobile-canvas`: light surface, fixed shaped bottom navigation, sticky topbar.
- `.ambient-layer`: present, fixed, viewport-height, opacity `0.42`, z-index `0`.
- `.signal-field-shell`: present and covers the viewport.
- `canvas.signal-field`: present, viewport-sized, internal pixels expand with DPR.
- `.route-veil`: rendered continuously and re-keyed for each navigation transition.
- Root route content receives `route-{direction}` and `transition-root`.

### Production Vue Before This Task

- `.mobile-canvas`, topbar and shaped bottom navigation are present.
- `.ambient-layer`: missing.
- `.signal-field-shell`: missing.
- `canvas.signal-field`: missing; total Canvas count on home is zero.
- `.route-veil`: missing.
- Route host only uses Vue `Transition` classes with generic 6-8px motion.
- Static stage image exists but is hidden at mobile breakpoints, so it cannot replace the missing mobile signal field.

## Exact Canvas Contract

- Four waveform lines with per-line amplitude, frequency and speed.
- Twenty-eight deterministic particles with drift and pointer-distance alpha.
- 34px grid, vertical scan band and pointer focus ring.
- Pointer target eases at `0.065` per frame.
- Light colors:
  - green `rgba(0, 126, 84, .46)`
  - blue `rgba(0, 104, 204, .34)`
  - coral `rgba(218, 62, 52, .24)`
- Dark colors:
  - green `rgba(84, 255, 181, .72)`
  - blue `rgba(55, 157, 255, .5)`
  - coral `rgba(255, 91, 75, .34)`
- DPR and total pixels are capped to avoid oversized Android WebView buffers.
- Reduced motion draws a fixed timestamp and does not schedule another frame.
- Animation pauses while `document.hidden` and restarts on visibility restoration.

## Route Motion Contract

- Root order determines `forward`, `back` or `still`.
- Root transition:
  - 360ms dark veil with signal-green rule.
  - 280ms content directional motion.
- Secondary transition:
  - no veil.
  - 170-180ms opacity transition.
- Sticky header layer remains above content and below no transient overlay that accepts input.

### Live Interaction Measurements

- Home -> Markets:
  - app stage: `data-route-direction="forward"`, `data-transition-tier="root"`.
  - host: `view-stack route-forward transition-root`, animation `root-forward`.
  - veil: `route-veil route-veil-root`, `data-direction="forward"`, animation `veil-presence`.
  - signal Canvas remains mounted.
- Markets -> Home:
  - app stage and veil direction switch to `back`.
  - host animation switches to `root-back`.
- Home -> Spot:
  - app stage switches to `data-motion-zone="protected"`.
  - mobile canvas switches to `data-surface="protected"`.
  - `.ambient-layer` and `canvas.signal-field` are both removed.
  - the root-direction veil remains active for the column change.

## Implementation Direction

- Add a self-contained Vue `SignalField` component; no third-party dependency.
- Add a reactive route-motion state in `core/navigation.ts` so router hooks and `App.vue` share one source.
- Keep Vue `Transition` for component lifecycle, but align its CSS and shell data attributes with the prototype contract.
- Render the signal field only for expressive root routes.
- Add structural tests that prevent future removal of the runtime layers and unit tests for route direction classification.

## Risks and Mitigations

- Battery/GPU load: cap DPR/pixels, pause hidden documents, clean up listeners and animation frames.
- Android WebView resize churn: use bounded resize handling and skip zero-sized measurements.
- Background covering content: preserve the prototype z-index contract and `pointer-events: none`.
- Motion sickness: honor `prefers-reduced-motion` in both Canvas and CSS.
