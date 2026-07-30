# Integrated Admin And API Image Research

## Repository Findings

- `web/` is the React/Vite administration and agent portal.
- The frontend uses browser history routes such as `/login`, `/admin/*`, and `/agent/*`.
- Frontend API requests already use same-origin absolute paths under `/admin/api/v1`,
  `/agent/api/v1`, and `/api/v1`; public WebSockets use `/ws/*`.
- The Rust router exposes `/health`, `/api/v1/*`, `/admin/api/v1/*`,
  `/agent/api/v1/*`, `/ws/*`, `/events/*`, `/docs`, and OpenAPI JSON endpoints.
- The current image runs as UID/GID `10001:10001`, exposes `8080`, includes
  `exchange-api` and `exchange-migrate`, and uses the same image for the one-shot
  migration service.

## Chosen Architecture

- Add a Node build stage that runs `npm ci` and `npm run build` in `web/`.
- Install Nginx and Tini in the existing Debian runtime image.
- Copy `web/dist` into the runtime image and serve it at `/`.
- Bind Rust only to `127.0.0.1:8081`; bind Nginx to public container port `8080`.
- Proxy all known backend prefixes before the SPA fallback so existing mobile,
  PC, admin, agent, WebSocket, health, and OpenAPI URLs remain compatible.
- Run Nginx and Rust under a small supervisor script. If either process exits,
  terminate the other and return the failing status.
- Keep Tini as the image entrypoint and the supervisor as the default command.
  Compose command overrides therefore continue to run `exchange-migrate`
  directly without starting Nginx.

## Rejected Alternatives

- Serving the frontend from Rust would not satisfy the requested Nginx integration.
- Running Nginx as a second Compose service would create two runtime images and
  would not satisfy the single-image requirement.
- Moving every API behind a new `/api` prefix would break existing PC and mobile clients.
- Running Rust and Nginx without supervision could leave a partially running,
  unhealthy container that Docker restart policies would not automatically replace.
