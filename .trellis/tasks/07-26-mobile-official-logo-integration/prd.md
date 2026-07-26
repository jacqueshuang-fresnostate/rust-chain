# Integrate Official HIPPO Logos Into The Mobile Prototype

## Goal

Replace temporary HIPPO wordmarks and placeholder brand marks in the public
mobile Sites prototype with the three official logo assets supplied by the
user, while preserving the existing Signal Theatre art direction, responsive
layout, complete route set, and financial interaction behavior.

## What I Already Know

- The user supplied three official PNG assets:
  - a 1000x250 transparent landscape lockup with an illustrated symbol panel;
  - a 2048x2048 white-background full lockup;
  - a 500x500 transparent compact lockup suitable for dark surfaces.
- The prototype currently renders a temporary `H` mark and text `HIPPO` in the
  mobile top bar.
- The desktop exhibition stage currently uses `signal-theatre.png` as its main
  bitmap artwork.
- Open Graph and X metadata currently reuse `signal-theatre.png`.
- The existing public Sites deployment is version 6 and must remain public.
- The user considers the current light theme too muted and dark.

## Assumptions

- Treat the supplied artwork as authoritative and do not redraw, recolor,
  distort, or separate its component shapes.
- Use the compact transparent lockup in the mobile top bar and dark UI.
- Use the landscape lockup as the desktop exhibition-stage brand signal.
- Use the square white-background lockup for link previews where a self-contained
  light composition is appropriate.
- Preserve the textual `HIPPO` product name in metadata and accessible labels.
- Rebalance the light theme around clean white, cool silver, graphite and vivid
  brand accents instead of the current beige-grey palette.

## Requirements

- Store all three assets under stable, descriptive paths in `public/`.
- Replace the temporary `H` brand mark and adjacent text with an accessible
  image-based brand button.
- Add the official landscape lockup to the desktop exhibition stage without
  obscuring the Signal Theatre artwork or market tape.
- Update Open Graph and X preview metadata to use the official square lockup.
- Keep image aspect ratios stable at every viewport and prevent layout shift.
- Ensure the logo remains legible in dark and light themes.
- Preserve Lucide-only functional icon usage; the official logo is a brand
  asset, not an interface icon.
- Preserve all routes, root-column responsibilities, form behavior, and the
  existing public access mode.
- Brighten the major light-theme surfaces, including portfolio, market brief,
  market index and asset overview, without weakening financial status contrast.

## Acceptance Criteria

- [x] All three official logo files are present in `public/` with stable names.
- [x] The mobile top bar uses the official compact logo and no temporary `H`
      mark.
- [x] The desktop stage visibly includes the official landscape logo.
- [x] Open Graph and X metadata use the official square logo asset.
- [x] Logos are not stretched, clipped, or illegible at 390x844 and 1440x900.
- [x] The prototype has no horizontal overflow or new console errors.
- [x] The light theme uses crisp neutral surfaces and vivid accents rather than
      beige-grey or large near-black panels.
- [x] Lint, production build, focused tests, and static asset checks pass.
- [x] The exact validated source is published as a new public Sites version.

## Definition Of Done

- Brand asset integration is complete and visually verified.
- Regression checks confirm the product flows remain unchanged.
- Task and progress records are updated.
- The public production deployment is refreshed and verified.

## Out Of Scope

- Redesigning or editing the supplied logo artwork.
- Changing the product information architecture or trading workflows.
- Integrating the logos into `mobile/src`, PC, backend, or admin applications.
- Connecting the prototype to real accounts or transactions.

## Technical Notes

- Primary files:
  - `mobile/sites-prototype/app/page.tsx`
  - `mobile/sites-prototype/app/globals.css`
  - `mobile/sites-prototype/app/layout.tsx`
  - `mobile/sites-prototype/tests/rendered-html.test.mjs`
- Asset target directory: `mobile/sites-prototype/public/`.
- Reuse the existing Sites project ID from `.openai/hosting.json`.
