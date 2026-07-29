# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.92.0

FROM rust:${RUST_VERSION}-bookworm AS builder

WORKDIR /workspace

RUN apt-get update \
    && apt-get install --yes --no-install-recommends libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/workspace/target,sharing=locked \
    cargo build --locked --release --bin exchange-api --bin exchange-migrate \
    && install -Dm755 target/release/exchange-api /out/exchange-api \
    && install -Dm755 target/release/exchange-migrate /out/exchange-migrate

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 exchange \
    && useradd --system --uid 10001 --gid exchange --no-create-home \
        --home-dir /nonexistent --shell /usr/sbin/nologin exchange \
    && install -d -o exchange -g exchange /app/uploads

COPY --from=builder /out/exchange-api /usr/local/bin/exchange-api
COPY --from=builder /out/exchange-migrate /usr/local/bin/exchange-migrate

ENV APP_HOST=0.0.0.0 \
    APP_PORT=8080 \
    RUST_LOG=info

WORKDIR /app
USER 10001:10001

EXPOSE 8080
STOPSIGNAL SIGTERM

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=5 \
    CMD ["curl", "--fail", "--silent", "--show-error", "--max-time", "5", "http://127.0.0.1:8080/health"]

CMD ["/usr/local/bin/exchange-api"]
