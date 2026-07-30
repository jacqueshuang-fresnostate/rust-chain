# 1Panel Compose Research

## Repository Contracts

- The image contains `/usr/local/bin/exchange-api` and
  `/usr/local/bin/exchange-migrate`.
- The API listens on `0.0.0.0:8080` and exposes `GET /health`.
- The runtime user is `10001:10001`.
- Required API keys are `DATABASE_URL`, `MONGODB_URI`,
  `MONGODB_DATABASE`, `REDIS_URL`, `RABBITMQ_URL`, `JWT_SECRET`, and
  `CREDENTIAL_ENCRYPTION_KEY`.
- The migration runner requires only `DATABASE_URL`.

## 1Panel Contracts

The current 1Panel v2 documentation supports creating a Compose through the
web editor, selecting an existing Compose path, or using a stored template.
1Panel-managed Compose deployments support lifecycle operations, and current
container Compose handling supports environment files.

The built-in `1panel-network` is the default integration point for separately
installed application containers. The deployment must still allow a custom
network name because installations and remote dependency topologies differ.

## Deployment Decisions

- Keep external service addresses in full connection URLs so passwords,
  database indexes, authentication databases, TLS parameters, and remote hosts
  are not reconstructed in YAML.
- Bind the host API port to `127.0.0.1` by default to avoid bypassing the
  1Panel reverse proxy. Operators can use `0.0.0.0` or omit public routing only
  when their topology requires it.
- Preserve a named uploads volume because it is application-owned data and the
  image runs as a non-root user.
- Treat a successful, exited migration container as the expected steady state.

## Validation Result

- Compose expansion contains exactly `api` and `migrate`.
- Both services resolve to the same GHCR image and external network.
- Migration receives only `DATABASE_URL` and `RUST_LOG`.
- API retains every required backend environment key and the
  `service_completed_successfully` migration gate.
- Only API port `8080` is published, bound to `127.0.0.1` by default.
- Only the application uploads volume is defined.
- Missing `DATABASE_URL` fails Compose interpolation before deployment.
- External network name, host bind address, and host port overrides expand
  correctly while the container port remains `8080`.
