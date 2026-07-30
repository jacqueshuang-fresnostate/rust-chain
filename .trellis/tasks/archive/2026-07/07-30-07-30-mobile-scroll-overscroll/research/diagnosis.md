# Scroll Boundary Diagnosis

## Findings

- `App.vue` uses the document as the root vertical scroll owner. `.app-route-host` and route layers do not create another full-page vertical scroller.
- `mobile/src/styles/base.css` currently applies only `overscroll-behavior-x: none` to `.app-frame`.
- Android WebView can therefore render its native vertical stretch affordance when document scrolling reaches either boundary.
- Existing confirmation and workflow sheets own intentional `overflow-y: auto` regions with `overscroll-behavior: contain`; those local contracts must stay unchanged.
- A root `html, body { overscroll-behavior: none; }` rule suppresses boundary effects and scroll chaining without disabling ordinary scrolling.

## Guardrails

- Do not add a global `touch-action` rule because chart and horizontal gesture surfaces rely on browser pointer arbitration.
- Do not add `overflow-y: hidden`, fixed document heights, or JavaScript `touchmove` cancellation.
- Keep route motion, sticky header geometry, launch-intro scroll locking, and nested sheet scrolling unchanged.
