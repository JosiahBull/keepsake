# syntax=docker/dockerfile:1

ARG RUST_VERSION=1
ARG NODE_VERSION=24

FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-trixie AS spec
RUN apt-get update && apt-get install -y --no-install-recommends cmake clang && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY .sqlx .sqlx
COPY packages/core packages/core
COPY packages/server packages/server
COPY packages/mcp packages/mcp
COPY packages/cli packages/cli
RUN mkdir -p packages/client && cargo run --locked --package keepsake-server --bin gen-openapi

FROM --platform=$BUILDPLATFORM node:${NODE_VERSION}-trixie-slim AS frontend
WORKDIR /app
RUN corepack enable
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY packages/web/package.json packages/web/package.json
COPY packages/client/package.json packages/client/package.json
RUN pnpm install --frozen-lockfile
COPY packages/web packages/web
COPY packages/client packages/client
COPY --from=spec /app/packages/client/openapi.json packages/client/openapi.json
RUN pnpm --filter @keepsake/client generate && pnpm --filter @keepsake/web build

FROM rust:${RUST_VERSION}-trixie AS backend
RUN apt-get update && apt-get install -y --no-install-recommends cmake clang && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY .sqlx .sqlx
COPY packages/core packages/core
COPY packages/server packages/server
COPY packages/mcp packages/mcp
COPY packages/cli packages/cli
RUN cargo build --locked --release --package keepsake-server --bin keepsake-server --package keepsake-cli --bin keepsake

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
RUN groupadd --gid 10001 keepsake && useradd --uid 10001 --gid keepsake keepsake \
    && mkdir -p /data && chown keepsake:keepsake /data
WORKDIR /app
COPY --from=backend --chown=root:root --chmod=0555 /app/target/release/keepsake-server /usr/local/bin/keepsake-server
COPY --from=backend --chown=root:root --chmod=0555 /app/target/release/keepsake /usr/local/bin/keepsake
COPY --from=frontend --chown=root:root /app/packages/web/dist /app/web
ENV DATABASE_URL=sqlite:/data/keepsake.db BIND_ADDR=0.0.0.0:8080 WEB_DIR=/app/web RUST_LOG=info
USER keepsake
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/keepsake-server", "--health-check"]
ENTRYPOINT ["keepsake-server"]
