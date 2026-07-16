# PC端图片缓存优化

## Goal

Optimize PC image loading so repeated visits do not fetch the same remote images every time. The main targets are brand logos, asset / trading pair logos, news banners, Launchpad / finance images, and QR-code style image requests.

## What I Already Know

- PC image usage is spread across shared components and pages, including `BrandLogo`, `PairLogo`, `News.vue`, `Launchpad.vue`, `Finance.vue`, `Assets.vue`, `Recharge.vue`, and security/KYC previews.
- Replacing every `<img>` with a new component would be broad and easy to miss.
- The PC app is Vite + Vue. There is currently no service worker registration in `pc/src/main.ts`.
- A root-scoped service worker can intercept all same app image requests without changing each page.

## Requirements

- Add a PC-side image cache that avoids repeated network requests for already cached image responses.
- Cache only safe `GET` image requests, identified by `request.destination === "image"` or common image file extensions.
- Support remote HTTP(S) images and local app-served images.
- Do not cache API JSON, JS, CSS, HTML, websocket, POST, or non-image requests.
- Limit the cache size so it does not grow without bound.
- Keep the page working if service workers or Cache Storage are unavailable.

## Acceptance Criteria

- [ ] PC app registers an image-cache service worker when the browser supports it.
- [ ] The service worker responds from Cache Storage for cached image requests.
- [ ] The service worker fetches and stores first-time image requests in the background.
- [ ] Cache pruning keeps a fixed maximum number of image entries.
- [ ] A static regression test covers registration and image-only caching behavior.
- [ ] `npm --prefix pc run type-check` passes.

## Definition of Done

- Tests added or updated for the caching behavior.
- Type-check passes.
- Progress is recorded in `docs/superpowers/PROGRESS.md`.

## Technical Approach

Use a root-scoped service worker file served from `pc/public/image-cache-sw.js`, then register it from `pc/src/main.ts` after app mount. The worker uses a stale-while-revalidate strategy for image requests:

- If cached, return the cached image immediately and refresh the cache in the background.
- If not cached, fetch from the network and cache the response.
- Cache opaque cross-origin image responses as well as normal successful image responses.
- Keep a maximum number of cached image entries.

## Decision (ADR-lite)

**Context**: Image URLs are used across many PC pages and shared components. Page-by-page conversion would be broad and fragile.

**Decision**: Use a service worker as the single image caching layer.

**Consequences**: This keeps page code mostly untouched and covers future `<img>` usage automatically. The cache is best-effort: browsers that disable service workers continue normal image loading.

## Out of Scope

- Backend `Cache-Control` header changes.
- Image resizing, CDN migration, or format conversion.
- API response caching.
- Rewriting every image to a dedicated cached image component.

## Technical Notes

- `pc/src/main.ts` is the registration point.
- `pc/public/image-cache-sw.js` will be served from the app root so its default scope can cover the PC app.
- `pc/tests` already has source-scanning tests for PC behavior; add a focused static test for image cache registration and worker behavior.
