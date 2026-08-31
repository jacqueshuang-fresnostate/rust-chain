# Admin Backend Origin and Integrated Image Contract

## 1. Scope / Trigger

Apply this contract whenever changing `web/src/config/backend.ts`, Admin Vite configuration,
the root `Dockerfile`, `.dockerignore`, the Docker image workflow, Nginx proxy paths, or a Compose
file. The integrated image serves the Admin SPA and Rust API behind one Nginx origin; a Vite API
mode error therefore makes the whole published image unusable before login.

## 2. Signatures

The Admin build accepts exactly these public compile-time inputs:

```text
VITE_API_SAME_ORIGIN = "true" | "false"
VITE_API_BASE_URL    = HTTP(S) Origin | omitted
```

`resolveBackendRuntimeConfig(environment, { production })` returns:

```ts
type BackendRuntimeConfig = {
  apiBaseUrl: string;
  mode: 'absolute' | 'same-origin';
};
```

The integrated root image owns this build signature:

```dockerfile
ARG VITE_API_SAME_ORIGIN=true
RUN VITE_API_SAME_ORIGIN="${VITE_API_SAME_ORIGIN}" npm run build
```

## 3. Contracts

- `VITE_*` values are consumed by Vite while static JavaScript is built. Docker runtime `ENV`,
  `docker run --env`, and Compose `environment` cannot change an existing bundle.
- Same-origin mode requires `VITE_API_SAME_ORIGIN=true` and an omitted/empty
  `VITE_API_BASE_URL`. REST URLs stay relative and WebSocket URLs derive `ws:` or `wss:` from the
  browser page origin.
- Absolute mode requires `VITE_API_SAME_ORIGIN=false` and a complete HTTP(S) Origin without
  credentials, query, hash, or path. Production non-loopback origins require HTTPS.
- `web/src/config/backend.ts` remains fail-closed. Do not add an implicit missing-value fallback.
- The integrated image always defaults to same-origin and must not inject `VITE_API_BASE_URL`.
  Scope the Vite assignment to the Admin build command; do not persist it in a later Docker stage.
- `.dockerignore` excludes `web/.env` and its variants. A developer's ignored local endpoint must
  never affect GHCR image output.
- Nginx continues to proxy `/api/v1/*`, `/admin/api/v1/*`, `/agent/api/v1/*`, `/ws/*`, and
  `/events/*` to the loopback Rust process.

## 4. Validation & Error Matrix

| Input / deployment state | Required result |
|---|---|
| Flag missing or not `true`/`false` | Throw `VITE_API_SAME_ORIGIN 必须显式设置为 true 或 false` |
| `true` plus non-empty base URL | Throw `同源模式不得同时设置 VITE_API_BASE_URL` |
| `false` plus empty base URL | Throw `非同源模式必须设置 VITE_API_BASE_URL` |
| Base URL is not HTTP(S) or contains a path | Reject before application startup |
| Production non-loopback HTTP base URL | Reject; production absolute mode requires HTTPS |
| Integrated Docker build with no build override | Compile same-origin mode successfully |
| Compose/runtime sets either Vite API variable | Contract-test failure; remove the runtime setting |
| Docker workflow overrides same-origin to false or supplies a base URL | Contract-test failure |

## 5. Good / Base / Bad Cases

- **Good**: the default GHCR build uses the Dockerfile argument, produces relative Admin API
  requests, and reaches Rust through the image's Nginx proxy.
- **Base**: a separately hosted Admin explicitly builds with `false` plus one HTTPS API Origin.
- **Bad**: a build omits the flag and still publishes because Vite compilation succeeds; the
  browser then throws during module initialization.
- **Bad**: Compose contains `VITE_API_SAME_ORIGIN=true`; this looks configured but has no effect on
  the already-built JavaScript.
- **Bad**: the Docker context copies an ignored local `web/.env`, making developer and CI images
  resolve different API origins.

## 6. Tests Required

After changing this boundary, run all of the following:

```bash
cargo test --test docker_image_contract
npm --prefix web run test:production-policy
npm --prefix web run typecheck
VITE_API_SAME_ORIGIN=true VITE_API_BASE_URL= npm --prefix web run build
npm --prefix web run budget
git diff --check
```

Assertions must prove:

- the Docker build argument precedes and is scoped to the single Admin `npm run build` step;
- Docker runtime stages, Compose files, and the image workflow do not override the contract;
- `.dockerignore` excludes local Admin env files;
- `buildApiUrl()` remains relative in same-origin mode;
- `buildWebSocketUrl()` derives `ws:`/`wss:` from the page origin;
- a production preview mounts the Admin root and issues API requests to the same page origin
  without the missing-flag exception.

## 7. Wrong vs Correct

### Wrong

```dockerfile
RUN npm run build
ENV VITE_API_SAME_ORIGIN=true
```

The first line builds an invalid bundle; the second line is too late and leaks a misleading
runtime setting into the image.

```yaml
services:
  api:
    environment:
      VITE_API_SAME_ORIGIN: "true"
```

Compose starts already-built static files, so this setting does not repair them.

### Correct

```dockerfile
FROM node:24-bookworm-slim AS web-builder
ARG VITE_API_SAME_ORIGIN=true
# install and copy steps omitted
RUN VITE_API_SAME_ORIGIN="${VITE_API_SAME_ORIGIN}" npm run build
```

This makes the integrated image's same-origin choice explicit at the only phase where Vite can
consume it while preserving fail-closed behavior for every other deployment.
