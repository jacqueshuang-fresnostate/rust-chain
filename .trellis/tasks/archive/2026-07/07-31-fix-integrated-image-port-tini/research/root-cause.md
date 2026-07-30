# Integrated Image Startup Root Cause

## Observed Failure

- The image starts Nginx on `0.0.0.0:8080`.
- Rust still accepts `APP_HOST` and `APP_PORT` from the deployment environment.
- A stale deployment value of `APP_PORT=8080` makes Rust race Nginx for the same socket and exit
  with `Address already in use`.
- The Tini warning reports a non-PID-1 Tini process, which means an outer init process wraps the
  image entrypoint.

## Design Constraint

The Nginx upstream is statically configured as `127.0.0.1:8081`; therefore the Rust address is not
an operator-configurable value in the integrated runtime. Allowing environment overrides creates a
configuration that can never be consistent with the bundled Nginx config.

## Fix

- The default supervisor owns and exports `APP_HOST=127.0.0.1` and `APP_PORT=8081` before starting
  Rust.
- Tini starts with `-s`, registering as a child subreaper whether it is PID 1 or nested below a
  platform-provided init.
- Direct command overrides still pass through Tini and bypass the supervisor, so migration behavior
  remains unchanged.

## Validation Strategy

- Build the image from the current source.
- Start the full dependency stack with an override that injects the stale `APP_HOST=0.0.0.0` and
  `APP_PORT=8080` values plus an outer Docker init.
- Assert Tini warning absence, Rust listener `127.0.0.1:8081`, Nginx listener `0.0.0.0:8080`,
  migration exit `0`, container health, and `/health` response.
- Override the image command and assert no Nginx process starts.
