# Cloudflare Turnstile mobile widget options

Primary references:

- https://developers.cloudflare.com/turnstile/get-started/client-side-rendering/widget-configurations/
- https://developers.cloudflare.com/turnstile/get-started/client-side-rendering/

## Findings

- Explicit rendering is the supported approach for SPAs that control widget creation timing.
- `size` supports `normal`, `flexible`, and `compact`; `flexible` lets Managed or Non-Interactive widgets use the available container width and is the preferred mobile layout.
- `theme` supports `auto`, `light`, and `dark`. The application can pass its own active theme during explicit rendering.
- `appearance: always` preserves visible verification feedback from page load and avoids hiding the security step unexpectedly.
- The widget container must remain available after the script loads; responsive styling belongs on the outer application surface, while Cloudflare owns the iframe internals.
- Turnstile tokens expire after five minutes and are single-use, so expired/reset callbacks must clear the local token without displaying a stale completed state.

## Repo mapping

- Keep the current explicit `turnstile.render(element, options)` integration.
- Add `size: 'flexible'`, application-derived `theme`, `appearance: 'always'`, and the current locale.
- Re-render on application theme or locale change because rendered iframe theme/language is not owned by Vue CSS.
- Style only the surrounding validation panel and container; never scale the iframe with CSS transforms.
