# Launch Motion Design

## Existing Shell

- `App.vue` owns the persistent `.app-stage > .mobile-canvas` shell.
- Root route motion already uses directional hard-edged veils.
- `SignalField` owns the long-running ambient Canvas and reduced-motion
  behavior for expressive root pages.
- The official landscape Logo is a tracked `1000x250` PNG and is already
  bundled by Vite.
- Existing shell layers stop at overlays/PWA status; the launch layer needs an
  explicit topmost, temporary layer without changing persistent layer order.

## Sequence

1. Render an opaque cool-black screen with a 34px technical grid and safe-area
   aware framing.
2. Reveal the official HIPPO lockup through a horizontal clip and restrained
   scale settle.
3. Draw three signal rails and advance a monospace acquisition counter.
4. Flash one precise coral scan edge.
5. Split the upper and lower shutters away while the lockup resolves into the
   already rendered application.

The sequence uses transforms, opacity, clip-path, and scale only. It does not
animate layout dimensions and does not create a second Canvas loop.

## Session And Cleanup

- Key: `hippo_mobile_launch_intro_v1`.
- Storage: `sessionStorage`, so a fresh app/browser session plays once and
  route navigation does not replay.
- Storage access is wrapped because WebView/privacy policies may throw.
- Reduced motion records the key and removes the layer synchronously after
  mount.
- A GSAP context owns all selectors; unmount reverts the context, kills the
  timeline, and removes the root scroll lock.
