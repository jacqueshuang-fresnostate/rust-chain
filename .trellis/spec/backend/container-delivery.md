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

- Entrypoint: `/usr/bin/tini -s --`, which forwards signals and acts as a child subreaper whether it
  is PID 1 or is nested below a platform-provided init.
- Default command: `/usr/local/bin/exchange-supervisor`, which starts and monitors Rust plus Nginx.
- Public listener: Nginx on `0.0.0.0:8080`.
- Internal listener: `/usr/local/bin/exchange-api` on `127.0.0.1:8081`.
- Migration process: `/usr/local/bin/exchange-migrate`, applying embedded SQLx migrations,
  bootstrapping the first administrator when `admin_users` is empty, and exiting.
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
- Before starting Rust, the supervisor must unconditionally export `APP_HOST=127.0.0.1` and
  `APP_PORT=8081`. Stale deployment environment values must never alter the integrated image's
  internal listener contract.
- Compose examples must not set `init: true` because the image already supplies Tini. Tini must use
  subreaper mode so an unavoidable 1Panel or Docker outer init remains supported without a
  non-PID-1 warning or child-reaping gap.
- Required API environment keys are `DATABASE_URL`, `MONGODB_URI`, `MONGODB_DATABASE`,
  `REDIS_URL`, `RABBITMQ_URL`, `JWT_SECRET`, and `CREDENTIAL_ENCRYPTION_KEY`.
- `CREDENTIAL_ENCRYPTION_KEY` must remain exactly 32 bytes and stable after encrypted data exists.
- The migration service always requires `DATABASE_URL`. It may also receive
  `BOOTSTRAP_ADMIN_USERNAME`, `BOOTSTRAP_ADMIN_PASSWORD`, and
  `BOOTSTRAP_ADMIN_ROLE_NAME`; these bootstrap values must never be passed to the API service.
- Missing or blank bootstrap values use the built-in defaults `admin`, `Qaz123456@`, and
  `super_admin`. Non-blank environment values override those defaults.
- Bootstrap normalizes the username with the shared auth helper, validates the password and role
  name, hashes the password with the shared Argon2 helper, and never logs or includes the plaintext
  password in errors.
- Bootstrap must serialize concurrent migration runners, then use one transaction to check for any
  administrator, create or reuse the requested role, and insert an active administrator. If any
  administrator already exists, it skips before creating a role and never changes an existing
  username, role, status, or password hash.
- Every acquired bootstrap named lock must be explicitly released on both success and failure
  paths. If the release query itself fails, close that physical MySQL connection instead of
  returning a potentially lock-owning session to the pool.
- The full-stack Compose persists MySQL, MongoDB, Redis, RabbitMQ, and `/app/uploads` in named
  volumes.
- The 1Panel Compose variant defines only `migrate` and `api`. It connects to independently
  managed MySQL, MongoDB, Redis, and RabbitMQ through full environment URLs, joins the external
  `${ONEPANEL_NETWORK:-1panel-network}`, and persists only the application-owned uploads volume.
- The 1Panel Compose defines `DATABASE_URL` and `RUST_LOG` once in a common YAML environment
  anchor consumed by both `migrate` and `api`. Operators who inline values instead of using the
  1Panel environment editor must replace the value in that common anchor, not only in the API
  environment map.
- The 1Panel migration service extends the common environment with bootstrap variables locally.
  Bootstrap variables must not be moved into the common or API environment anchor.
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
| Bootstrap variables are absent or blank | Use `admin`, `Qaz123456@`, and `super_admin` |
| A bootstrap username, password, or role name is invalid | Write no bootstrap rows, exit non-zero, and keep the API blocked |
| `admin_users` is empty and bootstrap credentials are valid | Create one active administrator with an Argon2 hash and the requested role |
| Any administrator already exists | Skip without creating a role or changing any administrator |
| Concurrent migration runners request bootstrap | Serialize bootstrap; create at most one administrator |
| Full-stack MySQL is unhealthy | Do not start migration or API |
| A migration fails | Migration exits non-zero and API remains blocked |
| Full-stack MongoDB, Redis, or RabbitMQ is unhealthy | API remains blocked |
| A 1Panel dependency URL or external network is invalid | Migration or API exits diagnostically; do not create a replacement dependency |
| The `web/` lockfile cannot complete a clean `npm ci` | Fail the image build; do not use an unlocked install |
| Rust is unavailable or API `/health` is non-200 | Nginx health route fails and the container becomes unhealthy |
| A browser opens `/login`, `/admin/*`, or `/agent/*` | Return the built SPA `index.html` |
| An API, WebSocket, event, or documentation path is requested | Proxy to Rust; never return the SPA fallback |
| Rust or Nginx exits | Supervisor terminates its sibling and exits so Compose can restart the container |
| Stale `APP_HOST=0.0.0.0` and `APP_PORT=8080` reach the default container | Supervisor replaces them and Rust listens only on `127.0.0.1:8081` |
| A platform-provided init wraps image Tini | Tini registers as a subreaper and emits no non-PID-1 warning |
| The migration command overrides the default command | Run only `exchange-migrate`; do not start Nginx or the supervisor |
| A pull request runs the workflow | Build both platforms on native runners, never log in or push |
| `main`, `v*`, or manual dispatch runs the workflow | Push both platform digests, then create and tag one manifest |
| Either architecture build runs | Use its native runner, local checkout context, and isolated cache; never set up QEMU |
| One distributed platform build fails | Do not finalize or publish an incomplete multi-architecture manifest |

### 5. Good/Base/Bad Cases

- Good: copy `docker-compose.env.example`, replace every placeholder, pull a pinned image tag,
  start the stack, and observe migration plus first-administrator bootstrap exit `0` followed by a
  healthy integrated container that serves both the admin SPA and API on port `8080`.
- Good (1Panel): install dependencies separately, connect them to the selected external network,
  provide full connection URLs through the Compose environment, observe migration exit `0`, and
  proxy only the healthy API through HTTPS.
- Base: run the local image with all four external dependencies and the required environment keys;
  access browser pages and API paths through the same origin.
- Bad: start the API and migration in parallel, use `depends_on` without health/completion
  conditions, let injected `APP_HOST` or `APP_PORT` override the supervisor-owned listener, expose
  Rust port `8081`, pass `BOOTSTRAP_ADMIN_PASSWORD` to the API, add redundant Compose `init: true`,
  commit `docker-compose.env`, run the final container as root, or serialize both architectures
  through QEMU on one x86 runner.
- Bad (1Panel): redefine MySQL or Redis in the application Compose, pass bootstrap credentials to
  the API, assume App Store container names, expose port `8080` publicly without intent, or treat a
  successfully exited migration container as a failure.

### 6. Tests Required

- Run `cargo fmt -- --check` and `cargo check --all-targets`.
- Run `npm --prefix web run build` and verify a clean Docker `npm ci` succeeds from the lockfile.
- Run `bash -n docker/supervise.sh`; assert it unconditionally exports `APP_HOST=127.0.0.1` and
  `APP_PORT=8081` before starting Rust; run `nginx -t` against the image configuration.
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
- Assert expanded `migrate` contains all three bootstrap variables while expanded `api` contains
  none of them.
- Build the image and assert UID/GID, both Rust executable paths, Tini entrypoint, supervisor command,
  built `index.html`, Nginx config, health check, exposed port `8080`, and writable runtime paths.
- Assert the image entrypoint is exactly `["/usr/bin/tini","-s","--"]` and a direct migration
  command override bypasses the supervisor and Nginx.
- Start a fresh Compose project and assert dependency health, migration exit `0`, every SQLx
  migration marked successful, exactly one configured administrator with a verifiable Argon2 hash,
  idempotent migration reruns, concurrent migration serialization, rollback of role creation after
  a forced administrator insert failure, named-lock release after both successful and failed
  bootstrap attempts, and access through Nginx to `GET /health`, `/login`, a deep admin route, an
  API or WebSocket route, and a test file under `/uploads/`.
- Inject stale `APP_HOST=0.0.0.0` and `APP_PORT=8080` values while starting the API with an outer
  Docker init. Assert Rust listens only on `127.0.0.1:8081`, Nginx owns `0.0.0.0:8080`, `/health`
  succeeds, and no non-PID-1 Tini warning is logged.
- Kill one supervised child and assert the complete container restarts and `/health` recovers.

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
ENTRYPOINT ["/usr/bin/tini", "-s", "--"]
CMD ["/usr/local/bin/exchange-supervisor"]
```

Nginx is the only public listener, Tini remains a subreaper even when nested, and the supervisor
treats Rust plus Nginx as one restartable application.

Bootstrap credentials have the same one-shot boundary:

```yaml
# Wrong: the long-running API inherits the bootstrap password.
x-common-environment: &common-environment
  BOOTSTRAP_ADMIN_PASSWORD: Qaz123456@

# Correct: only the migration process receives bootstrap overrides.
services:
  migrate:
    environment:
      BOOTSTRAP_ADMIN_PASSWORD: ${BOOTSTRAP_ADMIN_PASSWORD:-Qaz123456@}
```
