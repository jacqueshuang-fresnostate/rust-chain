# Container Delivery Contract

## Scenario: Publish And Run The Integrated Application Container

### 1. Scope / Trigger

- Apply this contract when changing `Dockerfile`, `.github/workflows/docker-image.yml`,
  `docker/nginx.conf`, `docker/supervise.sh`, `docker-compose.example.yml`,
  `docker-compose.1panel.example.yml`, the migration runner, or required runtime environment
  variables.
- The image contains the Rust backend and the `web/` admin/agent SPA. PC and mobile artifacts remain
  outside this image.

### 2. Signatures

- Entrypoint: `/usr/bin/tini --`, which remains PID 1 for signal forwarding and child reaping.
- Default command: `/usr/local/bin/exchange-supervisor`, which starts and monitors Rust plus Nginx.
- Public listener: Nginx on `0.0.0.0:8080`.
- Internal listener: `/usr/local/bin/exchange-api` on `127.0.0.1:8081`.
- Migration process: `/usr/local/bin/exchange-migrate`, applying embedded SQLx migrations and exiting.
- Health endpoint: `GET /health` returns HTTP 200 with `{"status":"ok"}`.
- Published image: `ghcr.io/jacqueshuang-fresnostate/rust-chain:<tag>`.
- Build workflow: native GitHub Actions matrix plus digest-based manifest finalization.

### 3. Contracts

- The runtime user is fixed to UID/GID `10001:10001`.
- The Node build stage must run `npm ci` against `web/package-lock.json`, build `web/`, and copy only
  `web/dist` into `/usr/share/nginx/html`. A clean lockfile install must succeed in Docker.
- Nginx owns container port `8080`; Rust must bind only to `127.0.0.1:8081`. Compose publishes
  container port `8080` and must not publish `8081`.
- Nginx serves `/uploads/` from `/app/uploads`, applies SPA history fallback to browser routes, and
  proxies `/health`, `/api/v1`, `/admin/api/v1`, `/agent/api/v1`, `/ws`, `/events`, and OpenAPI
  documentation paths to Rust.
- Proxy requests preserve `Host`, client address, forwarded protocol, and WebSocket Upgrade headers.
- Nginx PID and temporary paths must live under the exchange-owned `/tmp/exchange-nginx` tree so the
  complete runtime can remain non-root.
- The supervisor must terminate the sibling process and exit whenever Rust or Nginx exits. Compose
  restart policy then restarts the complete application boundary.
- Compose must not set `init: true`; the image already supplies Tini and nested init processes emit
  warnings and obscure the PID 1 contract.
- Required API environment keys are `DATABASE_URL`, `MONGODB_URI`, `MONGODB_DATABASE`,
  `REDIS_URL`, `RABBITMQ_URL`, `JWT_SECRET`, and `CREDENTIAL_ENCRYPTION_KEY`.
- `CREDENTIAL_ENCRYPTION_KEY` must remain exactly 32 bytes and stable after encrypted data exists.
- The migration service requires only `DATABASE_URL`; it must complete successfully before API start.
- The full-stack Compose persists MySQL, MongoDB, Redis, RabbitMQ, and `/app/uploads` in named
  volumes.
- The 1Panel Compose variant defines only `migrate` and `api`. It connects to independently
  managed MySQL, MongoDB, Redis, and RabbitMQ through full environment URLs, joins the external
  `${ONEPANEL_NETWORK:-1panel-network}`, and persists only the application-owned uploads volume.
- The 1Panel Compose defines `DATABASE_URL` and `RUST_LOG` once in a common YAML environment
  anchor consumed by both `migrate` and `api`. Operators who inline values instead of using the
  1Panel environment editor must replace the value in that common anchor, not only in the API
  environment map.
- The 1Panel API host port binds to `127.0.0.1` by default. Operators may instead route to the
  `hippo-exchange-api:8080` network alias or explicitly override the bind address.
- External dependency readiness is an operator responsibility in the 1Panel variant. The
  migration completion gate remains mandatory, and a migration failure must block API startup.
- The workflow maps `linux/amd64` to `ubuntu-24.04` and `linux/arm64` to
  `ubuntu-24.04-arm`, then builds them concurrently on native GitHub-hosted runners.
- The workflow must not install or use QEMU for these two platforms. The superseded single x86
  runner/QEMU build was cancelled after about 58 minutes while still compiling Rust crates; it did
  not fail because of a compiler error or GHCR authentication.
- A superseded reusable-builder attempt proved native ARM routing but failed with
  `unknown API capability source.git.checksum`; every platform job must therefore check out the
  repository and use local `context: .`.
- Both matrix jobs enable GitHub Actions cache in `max` mode with architecture-specific scopes.
- The pull-request matrix has only `contents: read`, sets `push: false`, does not request
  `packages: write`, and does not receive registry credentials.
- Each publish platform job adds `packages: write`, authenticates with `GITHUB_TOKEN`, and pushes
  one canonical image by digest. The manifest job runs only after both platform jobs succeed,
  downloads both digest artifacts, and applies branch, semver, SHA, and `latest` tags.

### 4. Validation & Error Matrix

| Condition | Required result |
|-----------|-----------------|
| `DATABASE_URL` is absent from the migration process | Exit non-zero with a configuration error |
| Full-stack MySQL is unhealthy | Do not start migration or API |
| A migration fails | Migration exits non-zero and API remains blocked |
| Full-stack MongoDB, Redis, or RabbitMQ is unhealthy | API remains blocked |
| A 1Panel dependency URL or external network is invalid | Migration or API exits diagnostically; do not create a replacement dependency |
| The `web/` lockfile cannot complete a clean `npm ci` | Fail the image build; do not use an unlocked install |
| Rust is unavailable or API `/health` is non-200 | Nginx health route fails and the container becomes unhealthy |
| A browser opens `/login`, `/admin/*`, or `/agent/*` | Return the built SPA `index.html` |
| An API, WebSocket, event, or documentation path is requested | Proxy to Rust; never return the SPA fallback |
| Rust or Nginx exits | Supervisor terminates its sibling and exits so Compose can restart the container |
| The migration command overrides the default command | Run only `exchange-migrate`; do not start Nginx or the supervisor |
| A pull request runs the workflow | Build both platforms on native runners, never log in or push |
| `main`, `v*`, or manual dispatch runs the workflow | Push both platform digests, then create and tag one manifest |
| Either architecture build runs | Use its native runner, local checkout context, and isolated cache; never set up QEMU |
| One distributed platform build fails | Do not finalize or publish an incomplete multi-architecture manifest |

### 5. Good/Base/Bad Cases

- Good: copy `docker-compose.env.example`, replace every placeholder, pull a pinned image tag,
  start the stack, and observe migration exit `0` followed by a healthy integrated container that
  serves both the admin SPA and API on port `8080`.
- Good (1Panel): install dependencies separately, connect them to the selected external network,
  provide full connection URLs through the Compose environment, observe migration exit `0`, and
  proxy only the healthy API through HTTPS.
- Base: run the local image with all four external dependencies and the required environment keys;
  access browser pages and API paths through the same origin.
- Bad: start the API and migration in parallel, use `depends_on` without health/completion
  conditions, expose Rust port `8081`, add Compose `init: true`, commit `docker-compose.env`, run
  the final container as root, or serialize both architectures through QEMU on one x86 runner.
- Bad (1Panel): redefine MySQL or Redis in the application Compose, hard-code secrets in YAML,
  assume App Store container names, expose port `8080` publicly without intent, or treat a
  successfully exited migration container as a failure.

### 6. Tests Required

- Run `cargo fmt -- --check` and `cargo check --all-targets`.
- Run `npm --prefix web run build` and verify a clean Docker `npm ci` succeeds from the lockfile.
- Run `bash -n docker/supervise.sh` and `nginx -t` against the image configuration.
- Parse the workflow and assert triggers, exact per-job permissions, native platform matrix,
  local checkout context, per-platform digest export, artifact merge, tags, cache scopes, registry
  authentication, and push policy.
- Assert `linux/amd64` resolves to `ubuntu-24.04`, `linux/arm64` resolves to
  `ubuntu-24.04-arm`, and no QEMU setup action remains.
- Run `docker compose --env-file docker-compose.env.example -f docker-compose.example.yml config`.
- Run `docker compose --env-file docker-compose.1panel.env.example
  -f docker-compose.1panel.example.yml config`.
- Assert the 1Panel result has exactly `api` and `migrate`, uses one image, references one external
  network, retains the migration completion gate, publishes only API port `8080`, and contains no
  MySQL, MongoDB, Redis, or RabbitMQ service/volume definitions.
- Assert expanded `api` and `migrate` environments contain identical `DATABASE_URL` and `RUST_LOG`
  values.
- Build the image and assert UID/GID, both Rust executable paths, Tini entrypoint, supervisor command,
  built `index.html`, Nginx config, health check, exposed port `8080`, and writable runtime paths.
- Start a fresh Compose project and assert dependency health, migration exit `0`, every SQLx
  migration marked successful, and access through Nginx to `GET /health`, `/login`, a deep admin
  route, an API or WebSocket route, and a test file under `/uploads/`.
- Kill one supervised child and assert the complete container restarts, PID 1 remains Tini, and
  `/health` recovers without a nested-Tini warning.

### 7. Wrong vs Correct

#### Wrong

```dockerfile
ENV APP_HOST=0.0.0.0 APP_PORT=8080
EXPOSE 8080 8081
CMD ["/usr/local/bin/exchange-api"]
```

This bypasses the admin frontend and Nginx, exposes the internal Rust listener, and loses coordinated
process shutdown.

#### Correct

```dockerfile
ENV APP_HOST=127.0.0.1 APP_PORT=8081
EXPOSE 8080
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/local/bin/exchange-supervisor"]
```

Nginx is the only public listener, Tini remains PID 1, and the supervisor treats Rust plus Nginx as
one restartable application.
