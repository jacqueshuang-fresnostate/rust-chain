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
- Migration process: `/usr/local/bin/exchange-migrate`, applying embedded SQLx migrations and,
  only when `BOOTSTRAP_MODE=create_admin`, bootstrapping the first administrator before exiting.
- Health endpoint: `GET /health` returns HTTP 200 with `{"status":"ok"}`.
- Published image: `ghcr.io/jacqueshuang-fresnostate/rust-chain:<tag>`.
- Build workflow: native GitHub Actions matrix plus digest-based manifest finalization.
- Runtime Turnstile policy endpoint:
  `GET /api/v1/auth/login/config -> { cf_turnstile_enabled, cf_turnstile_site_key }`.

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
- Runtime login Turnstile uses `CF_TURNSTILE_SECRET` (or the legacy
  `CF_TURNSTILE_SECRET_KEY`), `CF_TURNSTILE_SITE_KEY`,
  `CF_TURNSTILE_SITEVERIFY_URL`, and `CF_TURNSTILE_ENFORCE_TOKEN`. The integrated admin SPA must
  obtain the public Site Key from the login-config API; a Vite build-time key is fallback only.
- Turnstile is enabled when both a non-blank Secret and Site Key exist. The enforce flag does not
  control widget visibility. It only controls whether an existing `cf_clearance` cookie may skip
  the login token: `true` always requires `cf_turnstile_token`; `false` skips only when
  `cf_clearance` is present and otherwise still requires the token.
- The admin SPA reads `/api/v1/auth/login/config` before the equivalent admin path because a
  Cloudflare Managed Challenge may target `/admin/*`. The admin endpoint remains a fallback and
  must return the same policy.
- `CREDENTIAL_ENCRYPTION_KEY` must remain exactly 32 bytes and stable after encrypted data exists.
- The migration service always requires `DATABASE_URL`. It may also receive `BOOTSTRAP_MODE`,
  `BOOTSTRAP_ADMIN_USERNAME`, exactly one of `BOOTSTRAP_ADMIN_PASSWORD` or
  `BOOTSTRAP_ADMIN_PASSWORD_FILE`, and `BOOTSTRAP_ADMIN_ROLE_NAME`; bootstrap values must never be
  passed to the API service.
- Bootstrap is disabled when `BOOTSTRAP_MODE` is absent or `disabled`. Only the exact
  `create_admin` value enables it. In that mode a non-blank, non-default one-time password is
  mandatory; username and role may retain the non-secret `admin` and `super_admin` defaults.
- Bootstrap normalizes the username with the shared auth helper, validates the password and role
  name, hashes the password with the shared Argon2 helper, and never logs or includes the plaintext
  password in errors.
- Bootstrap must serialize concurrent migration runners, then use one transaction to check for any
  administrator, create or reuse the requested role, and insert an active administrator marked for
  mandatory password rotation. If any
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
- A schema-wide text metadata repair that issues DDL across all business tables must be deployed
  in a planned maintenance window with application writes stopped and a verified database backup.
  `ALTER TABLE` may wait for metadata locks and may rebuild tables or indexes; operators must
  inspect long-running transactions before starting the migration and monitor MySQL until it
  completes.
- Invalid UTF-8 bytes in a drifted binary text column must make the migration fail. The migration
  process must remain non-zero and keep the API blocked; operators must restore or clean the
  affected data deliberately, never enable replacement-character conversion.
- MySQL DDL implicit commits mean a failed schema-wide repair can leave earlier tables repaired
  while SQLx retains a `success=FALSE` dirty row. After resolving the lock or invalid bytes, verify
  that exact failed version, delete only its dirty row, and rerun the same immutable migration
  while writes remain stopped. Never mark the row successful or hand-write reverse DDL.
- A successful text metadata repair has no automatic down migration. An application-image rollback
  may keep the canonical repaired schema when compatibility is confirmed; exact database rollback
  must restore the verified pre-maintenance backup into an isolated target and switch only after
  consistency checks.
- The workflow maps `linux/amd64` to `ubuntu-24.04` and `linux/arm64` to
  `ubuntu-24.04-arm`, then builds them concurrently on native GitHub-hosted runners.
- The workflow must not install or use QEMU for these two platforms. The superseded single x86
  runner/QEMU build was cancelled after about 58 minutes while still compiling Rust crates; it did
  not fail because of a compiler error or GHCR authentication.
- A superseded reusable-builder attempt proved native ARM routing but failed with
  `unknown API capability source.git.checksum`; every platform job must therefore check out the
  repository and use local `context: .`.
- Both matrix jobs enable GitHub Actions cache in `max` mode with architecture-specific scopes.
- The Dockerfile must use the Dockerfile frontend bundled with the GitHub Buildx/BuildKit runner;
  do not add a remote `docker/dockerfile` `# syntax=` image reference because resolving that
  frontend creates an unnecessary Docker Hub authentication dependency before the build graph can
  start. Keep the stable BuildKit instructions already covered by the image contract, including
  cache mounts and `COPY --chmod`.
- The pull-request matrix has only `contents: read`, sets `push: false`, does not request
  `packages: write`, and does not receive registry credentials.
- Each publish platform job adds `packages: write`, authenticates with `GITHUB_TOKEN`, and pushes
  one canonical image by digest. The manifest job runs only after both platform jobs succeed,
  downloads both digest artifacts, and applies branch, semver, SHA, and `latest` tags.

### 4. Validation & Error Matrix

| Condition | Required result |
|-----------|-----------------|
| `DATABASE_URL` is absent from the migration process | Exit non-zero with a configuration error |
| `BOOTSTRAP_MODE` is absent or `disabled` | Apply migrations and create no administrator |
| Bootstrap mode is enabled but password is absent, blank, duplicated across env/file, or known-default | Write no bootstrap rows and exit non-zero |
| A bootstrap username, password, or role name is invalid | Write no bootstrap rows, exit non-zero, and keep the API blocked |
| `admin_users` is empty and bootstrap credentials are valid | Create one active administrator with an Argon2 hash and the requested role |
| Any administrator already exists | Skip without creating a role or changing any administrator |
| Concurrent migration runners request bootstrap | Serialize bootstrap; create at most one administrator |
| Full-stack MySQL is unhealthy | Do not start migration or API |
| A migration fails | Migration exits non-zero and API remains blocked |
| A schema-wide metadata repair waits on a long transaction | Keep writes stopped; identify and resolve the metadata-lock owner before retrying deployment |
| A binary text value is not valid UTF-8 | Migration fails; preserve the backup and clean the specific value without lossy replacement |
| A schema-wide repair partially commits before failing | Keep writes stopped; resolve the cause, delete only the verified `success=FALSE` row, and rerun the same immutable migration |
| Exact pre-repair database state is required | Restore the verified full backup into an isolated target; do not reverse columns in place |
| Full-stack MongoDB, Redis, or RabbitMQ is unhealthy | API remains blocked |
| A 1Panel dependency URL or external network is invalid | Migration or API exits diagnostically; do not create a replacement dependency |
| Turnstile Secret or Site Key is blank | Return `cf_turnstile_enabled=false`; do not require a token the client cannot render |
| Secret and Site Key are present, `CF_TURNSTILE_ENFORCE_TOKEN=true` | Return enabled config and require a valid token on every login |
| Secret and Site Key are present, enforce is false, no `cf_clearance` exists | Return enabled config and still require a valid token |
| Secret and Site Key are present, enforce is false, valid `cf_clearance` exists | The login token may be skipped |
| Cloudflare challenges `/admin/api/v1/auth/login/config` | Admin SPA uses the public login-config path and still renders the widget |
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

- Good: copy `docker-compose.env.example`, replace every placeholder, inject a one-time random
  bootstrap Secret only for the first deployment, pull a pinned image tag, start the stack, and observe migration plus first-administrator bootstrap exit `0` followed by a
  healthy integrated container that serves both the admin SPA and API on port `8080`.
- Good (1Panel): install dependencies separately, connect them to the selected external network,
  provide full connection URLs through the Compose environment, observe migration exit `0`, and
  proxy only the healthy API through HTTPS.
- Good (Turnstile): configure matching Site Key and Secret values, recreate the API container,
  confirm the public login-config response is enabled, and render the runtime Site Key in the admin
  login page even when `/admin/*` has a Managed Challenge rule.
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
- Bad (Turnstile): set only the Secret, use `CF_TURNSTILE_ENFORCE_TOKEN=false` as an enable/disable
  switch, rely only on a Vite build-time key, or fetch the admin-scoped config first behind a
  Cloudflare `/admin/*` challenge rule.

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
- Assert expanded `migrate` contains the bootstrap mode, username, password/password-file, and role variables while expanded `api` contains
  none of them.
- Assert the expanded 1Panel API environment contains matching Turnstile Secret/Site Key examples,
  the default enforce value is `true`, and the login policy is disabled if either half is absent.
- Unit-test the enforce/`cf_clearance` matrix. In the admin Web tests, assert the public config path
  is requested first, the admin path is a fallback, the runtime Site Key reaches
  `turnstile.render`, and the callback token reaches the login request.
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
- For a schema-wide metadata repair, run the complete migration set on fresh MySQL 8.4, execute the
  exact repair SQL against an already-correct schema, and run the drift regression against a
  separate real MySQL database. Verify canonical column/index metadata, stored text bytes, database
  and table defaults, and untouched BLOB payloads.
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
# Wrong: the long-running API inherits a bootstrap password.
x-common-environment: &common-environment
  BOOTSTRAP_ADMIN_PASSWORD: ${BOOTSTRAP_ADMIN_PASSWORD}

# Correct: only an explicitly enabled migration process receives the one-time Secret.

services:
  migrate:
    environment:
      BOOTSTRAP_MODE: ${BOOTSTRAP_MODE:-disabled}
      BOOTSTRAP_ADMIN_PASSWORD: ${BOOTSTRAP_ADMIN_PASSWORD:-}
      BOOTSTRAP_ADMIN_PASSWORD_FILE: ${BOOTSTRAP_ADMIN_PASSWORD_FILE:-}
```

Turnstile enablement must not be coupled to the clearance override:

```rust
// Wrong: false hides the widget and disables all token verification.
let enabled = secret.is_some() && site_key.is_some() && enforce_token;

// Correct: credentials enable the widget; enforce only controls the clearance exception.
let enabled = secret.is_some() && site_key.is_some();
let require_token = enabled && (enforce_token || !has_cf_clearance);
```

## Scenario: Pre-install Source Integrity Gate

### 1. Scope / Trigger

- Trigger: changing executable build configuration, release workflows, dependency-install steps, image builds, or the root P0 release gate.
- Applies to every CI job that can install dependencies, load frontend tooling, build an image, or publish an artifact.

### 2. Signatures

```text
checkout -> static source-integrity scan -> toolchain setup/cache/install/build
local release gate -> static scan + scanner tests -> language/toolchain checks
```

The scanner is a text-only program using the Python standard library. It does not import, require, transpile, or execute the files it inspects.

### 3. Contracts

- A clean checkout must be scanned before Node/Rust setup, cache restoration, dependency installation, Buildx setup, Docker build, or publishing.
- Every repository `package.json` is part of the static scan boundary. Automatic install lifecycle scripts (`preinstall`, `install`, `postinstall`, `prepare`, and related hooks), hidden pre/post hooks around protected release scripts, network/download commands, inline dynamic evaluation, encoded loaders, and malformed manifests are release-blocking.
- Executable frontend build configuration must be declarative. Network clients, child-process APIs, dynamic evaluation, detached processes, and unexplained long executable lines are release-blocking.
- `pc/package.json` must retain `build = "vite build"`; the local P0 gate must execute that production build after PC type-check and tests, so replacing it with a no-op cannot satisfy release evidence.
- Known incident hashes are explicit IOCs in the scanner and its tests. Removing the exact hash must not remove generic behavioral detection.
- CI and local release gates call the same scanner; a workflow-only shell fragment is not an independent source of truth.
- Scanner fixtures are inert text generated inside a temporary directory. Tests never import a malicious fixture.
- A clean source scan proves only repository state. Credential rotation, runner/cache rebuild, artifact invalidation, and historical investigation remain separate operational evidence.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| Known IOC hash | Non-zero exit with path and rule identifier |
| Long single-line executable payload in build config | Non-zero exit |
| Network or child-process capability in executable build config | Non-zero exit unless a narrow reviewed allowlist entry exists |
| Dynamic evaluation combined with network/process capability | Non-zero exit |
| Automatic npm install lifecycle or protected pre/post release hook | Non-zero exit |
| PC build script missing or replaced | Non-zero exit before dependency installation/build |
| Minimal PostCSS/Tailwind/Vite declaration | Pass without loading the config |
| Scanner cannot read a required path | Fail closed |

### 5. Tests Required

- Standard-library unit tests cover clean input, known hash, long line, dynamic evaluation plus network access, direct child-process use, package lifecycle hooks, malformed manifests, and the pinned PC build command.
- Workflow contract tests or static assertions prove every build/publish job scans immediately after checkout.
- `scripts/p0-release-gate.sh` runs the scanner and its tests before Rust or frontend commands.
- Only after the tracked build configuration is clean may PC type-check, tests, and production build run.

### 6. Wrong vs Correct

```yaml
# Wrong: dependencies and build configuration are loaded before inspection.
- run: npm ci
- run: python3 scripts/source_integrity_gate.py
```

```yaml
# Correct: the checkout is inspected before any tool or cache can execute repository code.
- uses: actions/checkout@v6
- run: python3 scripts/source_integrity_gate.py
- uses: actions/setup-node@v6
```
