# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.92.0
ARG NODE_VERSION=24

FROM node:${NODE_VERSION}-bookworm-slim AS web-builder

WORKDIR /workspace/web

COPY web/package.json web/package-lock.json ./

RUN --mount=type=cache,target=/root/.npm,sharing=locked \
    npm ci

COPY web ./

RUN npm run build

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
    && apt-get install --yes --no-install-recommends \
        bash \
        ca-certificates \
        curl \
        libssl3 \
        nginx \
        tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 exchange \
    && useradd --system --uid 10001 --gid exchange --no-create-home \
        --home-dir /nonexistent --shell /usr/sbin/nologin exchange \
    && install -d -o exchange -g exchange \
        /app/uploads \
        /tmp/exchange-nginx \
        /tmp/exchange-nginx/client_temp \
        /tmp/exchange-nginx/proxy_temp \
        /tmp/exchange-nginx/fastcgi_temp \
        /tmp/exchange-nginx/uwsgi_temp \
        /tmp/exchange-nginx/scgi_temp

COPY --from=builder /out/exchange-api /usr/local/bin/exchange-api
COPY --from=builder /out/exchange-migrate /usr/local/bin/exchange-migrate
COPY --chown=10001:10001 --from=web-builder /workspace/web/dist /usr/share/nginx/html
COPY --chmod=0644 docker/nginx.conf /etc/nginx/nginx.conf
COPY --chmod=0755 docker/supervise.sh /usr/local/bin/exchange-supervisor

ENV APP_HOST=127.0.0.1 \
    APP_PORT=8081 \
    RUST_LOG=info

WORKDIR /app
USER 10001:10001

EXPOSE 8080
STOPSIGNAL SIGTERM

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=5 \
    CMD ["curl", "--fail", "--silent", "--show-error", "--max-time", "5", "http://127.0.0.1:8080/health"]

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/local/bin/exchange-supervisor"]
