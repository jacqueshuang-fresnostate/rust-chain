# Container Delivery Contract

## Scenario: Publish And Run The Backend Container

### 1. Scope / Trigger

- Apply this contract when changing `Dockerfile`, `.github/workflows/docker-image.yml`,
  `docker-compose.example.yml`, the migration runner, or required runtime environment variables.
- The image contains the Rust backend only. PC, admin, and mobile artifacts are outside this image.

### 2. Signatures

- Default process: `/usr/local/bin/exchange-api`, listening on `0.0.0.0:8080`.
- Migration process: `/usr/local/bin/exchange-migrate`, applying embedded SQLx migrations and exiting.
- Health endpoint: `GET /health` returns HTTP 200 with `{"status":"ok"}`.
- Published image: `ghcr.io/jacqueshuang-fresnostate/rust-chain:<tag>`.
- Build workflow: `docker/github-builder/.github/workflows/build.yml@v1`.

### 3. Contracts

- The runtime user is fixed to UID/GID `10001:10001`.
- Required API environment keys are `DATABASE_URL`, `MONGODB_URI`, `MONGODB_DATABASE`,
  `REDIS_URL`, `RABBITMQ_URL`, `JWT_SECRET`, and `CREDENTIAL_ENCRYPTION_KEY`.
- `CREDENTIAL_ENCRYPTION_KEY` must remain exactly 32 bytes and stable after encrypted data exists.
- The migration service requires only `DATABASE_URL`; it must complete successfully before API start.
- Compose persists MySQL, MongoDB, Redis, RabbitMQ, and `/app/uploads` in named volumes.
- Both caller jobs request `linux/amd64,linux/arm64` with distributed builds enabled. The official
  builder maps `linux/amd64` to `ubuntu-24.04` and `linux/arm64` to `ubuntu-24.04-arm`, builds them
  concurrently on native GitHub-hosted runners, and finalizes one multi-architecture manifest.
- The workflow must not install or use QEMU for these two platforms. The superseded single x86
  runner/QEMU build was cancelled after about 58 minutes while still compiling Rust crates; it did
  not fail because of a compiler error or GHCR authentication.
- Both jobs enable the signed GitHub Actions cache in `max` mode with stable scope
  `backend-image` and grant `id-token: write` for OIDC signing.
- The pull-request caller has only `contents: read` and `id-token: write`, sets `push: false`, does
  not request `packages: write`, does not receive registry credentials, and disables image signing.
- The publish caller adds `packages: write`, sets `push: true`, authenticates to GHCR with
  `GITHUB_TOKEN`, and uses automatic signing to emit signed provenance. Pushes to `main`, `v*`
  tags, and manual dispatches retain the branch, semver, SHA, and `latest` tag contracts.

### 4. Validation & Error Matrix

| Condition | Required result |
|-----------|-----------------|
| `DATABASE_URL` is absent from the migration process | Exit non-zero with a configuration error |
| MySQL is unhealthy | Do not start migration or API |
| A migration fails | Migration exits non-zero and API remains blocked |
| MongoDB, Redis, or RabbitMQ is unhealthy | API remains blocked |
| API `/health` is non-200 | Container health becomes unhealthy |
| A pull request runs the workflow | Build both platforms on native runners, never log in or push |
| `main`, `v*`, or manual dispatch runs the workflow | Log in to GHCR, push generated tags, and emit signed provenance |
| Either architecture build runs | Use its native runner and the signed shared cache; never set up QEMU |
| One distributed platform build fails | Do not finalize or publish an incomplete multi-architecture manifest |

### 5. Good/Base/Bad Cases

- Good: copy `docker-compose.env.example`, replace every placeholder, pull a pinned image tag,
  start the stack, and observe migration exit `0` followed by a healthy API.
- Base: run the local image with all four external dependencies and the required environment keys.
- Bad: start the API and migration in parallel, use `depends_on` without health/completion
  conditions, commit `docker-compose.env`, run the final container as root, or serialize both
  architectures through QEMU on one x86 runner.

### 6. Tests Required

- Run `cargo fmt -- --check` and `cargo check --all-targets`.
- Parse the workflow and assert the official `build.yml@v1` reusable workflow, triggers, exact
  per-job permissions, native distributed platforms, tags, signed cache configuration, provenance,
  registry authentication, and push policy.
- Assert `linux/amd64` resolves to `ubuntu-24.04`, `linux/arm64` resolves to
  `ubuntu-24.04-arm`, and no QEMU setup action remains.
- Run `docker compose --env-file docker-compose.env.example -f docker-compose.example.yml config`.
- Build the image and assert UID/GID, both executable paths, health check, and writable uploads path.
- Start a fresh Compose project and assert dependency health, migration exit `0`, every SQLx
  migration marked successful, and host access to `GET /health`.

### 7. Wrong vs Correct

#### Wrong

```yaml
api:
  depends_on:
    - mysql
```

This only waits for container creation and permits the API to race database readiness and migrations.

#### Correct

```yaml
api:
  depends_on:
    mysql:
      condition: service_healthy
    migrate:
      condition: service_completed_successfully
```

This blocks API startup until MySQL is accepting requests and the schema is current.
