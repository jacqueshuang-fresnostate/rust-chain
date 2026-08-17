# PwaStatus current-surface audit

## Existing behavior to preserve

- `PwaStatus` is mounted once inside the mobile canvas.
- The root layer is pointer-transparent; only cards and controls are interactive.
- Offline may coexist with one of the main status cards.
- Main-state priority is update, install, offline-ready, then error.
- Install/update prompts are route allowlisted; global offline behavior remains truthful.

## Visual weaknesses

- The current card is flush to the entire 448px canvas and reads as another
  header row rather than a deliberate system surface.
- A square card, 3px status rail, generic border, and no elevation provide no
  material hierarchy over the route below it.
- The 20px icon, title, paragraph, and actions share nearly the same visual
  weight, so actionable update/install states do not feel intentional.
- There is no transition choreography or reduced-motion-specific contract.

## Existing patterns worth reusing

- Semantic light/dark tokens from `mobile/src/styles/base.css`.
- The Assets transfer sheet uses constrained fixed glass, inner glints, and
  state-safe `color-mix` without remote assets.
- Shared controls use 44px minimum targets and explicit focus rings.
- The shell reserves z-index 80 for overlays and 120 for launch intro.

## Chosen design

- Ethereal Glass texture with a Z-axis notification stack.
- Detached top island keeps PWA information visible without covering the Dock
  or introducing modal focus/scroll behavior.
- A real nested outer shell and inner panel create a double bezel. Ambient
  pseudo-elements add a restrained state-colored glow and micro-grid.
- Vue transition uses only transform and opacity; any blur is static on the
  fixed card, not animated on the scrolling route.
- Lucide remains mandatory because it is the product icon contract, despite
  generic redesign guidance preferring custom icon systems.

## Verification implications

- Update old tests that assert `border-radius: 0` and `box-shadow: none`.
- Add source assertions for the nested panel, state data, backdrop filter,
  custom cubic-bezier, 44px targets, reduced motion, safe top offset, and no
  full-screen blocking overlay.
- Browser-check light/dark at 320, 390, and 448 widths with forced local PWA
  states; verify `scrollWidth === innerWidth` and underlying routes remain
  pointer reachable outside the card.
