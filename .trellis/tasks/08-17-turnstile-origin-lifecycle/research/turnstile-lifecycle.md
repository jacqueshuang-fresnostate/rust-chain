# Turnstile SPA lifecycle research

## Live reproduction

- Reproduced with Ego Browser on 2026-08-17 at:
  - `https://hipoex.cllbmz.kdns.fr/login`
  - `https://hippo.cllbmz.kdns.fr/#/login`
- CDP `Log.entryAdded` confirmed the warning originates from
  `https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit`.
- Both pages loaded one API script and one widget container.
- A clean load ultimately produced a non-empty `cf-turnstile-response` token,
  proving that the sitekey/hostname path currently succeeds.
- Frame instrumentation showed the challenge iframe starts navigation from the
  application origin and is removed after a successful token. A short warning
  can therefore be emitted inside Cloudflare's script during that transition.
- Persistent warnings were reproduced after an extra widget was rendered and
  its frame became stale. This matches the application risk from an async
  initialization that continues after component cleanup.

## Repository findings

- The mobile and admin components each store their loader Promise inside the
  component instance. A route remount can inject a second script while the
  first copy is still loading.
- Both initializers await external work and then render without a generation or
  `HTMLElement.isConnected` check.
- Callbacks from an old widget are not scoped to the render that created them.
- The admin effect also depends on login scope even though the same login
  challenge can be retained across the admin/agent selector.

## Official lifecycle contract

Cloudflare's SPA guidance recommends:

1. explicit rendering for dynamic single-page applications;
2. waiting for `turnstile.ready()` before creating a widget;
3. retaining the returned widget ID;
4. resetting that exact ID after a request when the page remains active; and
5. calling `turnstile.remove(widgetId)` when the widget is no longer needed.

References:

- https://developers.cloudflare.com/turnstile/get-started/client-side-rendering/
- https://developers.cloudflare.com/turnstile/additional-configuration/hostname-management/

## Chosen approach

- Keep one module-scoped loader Promise in each independently built frontend.
- Reuse an existing matching API script and wait for `turnstile.ready()`.
- Add a monotonically increasing render generation to each component.
- Invalidate before cleanup; check generation and `isConnected` after every
  asynchronous boundary and inside every callback.
- If a render becomes stale immediately after creation, remove the returned ID
  synchronously.
- Do not intercept `postMessage`, suppress console output, or weaken server-side
  validation.
