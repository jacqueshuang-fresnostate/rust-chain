# Mobile Header Skeuomorphic Controls

## Goal

Refine every mobile header icon control into one coherent, tactile
skeuomorphic system. The shared PageHeader back/action controls are the
priority, followed by RootHeader theme/message controls and the independent
authentication and market-detail headers.

## What I Already Know

- The user explicitly requested a better back button and skeuomorphic header
  actions.
- All icons must remain Lucide icons and no emoji may be introduced.
- `PageHeader` owns safe navigation through `goBackOr`; this behavior must not
  change.
- Root headers are 64px high, shared secondary headers are 76px high, and
  sticky headers remain at z-index 70.
- Shared header controls use 44x44px tracks. The PageHeader action slot is
  wrapped by a separate 44x44px `secondary-header-action` element.
- Login, register, and market detail use independent headers with the same
  `.icon-button` class.

## Requirements

- Apply a consistent precision-instrument appearance to:
  - RootHeader theme and message buttons.
  - PageHeader back button and every icon button rendered in its `actions`
    slot.
  - Login and registration header back/language buttons.
  - Market detail header back/share buttons.
- Use circular 44x44px controls with:
  - a cool-neutral metal bezel;
  - a subtly convex face;
  - inset highlight and lower edge depth;
  - a restrained elevation shadow;
  - tactile press movement and shadow compression.
- Preserve the coral unread indicator and keep it visually attached to the
  message control.
- Provide distinct light and dark theme treatments without reintroducing
  `#0b1811` or `rgba(11, 24, 17, ...)`.
- Remove visible framing from the PageHeader action-slot wrapper so only the
  actual interactive button is rendered as a control.
- Explicitly center every Lucide SVG on both axes.
- Preserve visible cyan keyboard focus, disabled state clarity, and reduced
  motion behavior.
- Preserve every existing click handler, route, aria label, loading animation,
  PWA behavior, API integration, and header geometry.

## Acceptance Criteria

- [ ] RootHeader and representative PageHeader controls visually share the
      same skeuomorphic construction in both themes.
- [ ] Authentication and market-detail header controls use the same system.
- [ ] Every rendered header icon target is at least 44x44px.
- [ ] Button and SVG centers differ by no more than 0.5px per axis.
- [ ] The PageHeader action wrapper has no visible border/background.
- [ ] Active press state moves by 1px and compresses depth without changing
      layout.
- [ ] Focus-visible state has a complete visible cyan ring.
- [ ] Disabled buttons do not animate or imply availability.
- [ ] Sticky headers retain their current height and layer order after scroll.
- [ ] 320x720, 390x844, and 448x900 have no horizontal overflow.
- [ ] Type-check, unit tests, PWA build, and Tauri build pass.

## Definition Of Done

- Relevant source-contract tests cover the new selectors and interaction
  states.
- Browser screenshots and computed geometry are checked in light and dark
  themes.
- Mobile shell spec and progress log record the shared header-control contract.
- Work is committed and the Trellis task is archived.

## Out Of Scope

- Page body controls, forms, dialogs, bottom navigation, and business layout.
- Navigation behavior, API calls, DTOs, routes, or authentication flow.
- Replacing Lucide icons or changing logo artwork.
- Adding a new component library or runtime dependency.

## Technical Notes

- Prefer production-only overrides in
  `mobile/src/styles/prototype-parity.css`; the tracked prototype snapshot in
  `prototype-base.css` remains unchanged.
- PageHeader action buttons are slotted, so style the wrapper as a transparent
  alignment track and target its direct `.icon-button` child.
- Independent scoped header styles currently set only flat borders; shared
  production selectors need sufficient specificity to own the final tactile
  treatment.

