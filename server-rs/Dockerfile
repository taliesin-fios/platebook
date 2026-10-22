# syntax=docker/dockerfile:1
#
# Platebook — Rust API server (server-rs) + the built React client, one image.
#
# Build context is the REPOSITORY ROOT (the client lives outside server-rs):
#
#   docker build -f server-rs/Dockerfile -t platebook .
#
# The root railway.json and docker-compose.selfhost.yml select this Dockerfile
# (see server-rs/README.md#deploying).
#
# Only client/ and server-rs/ are copied in; server-rs/Dockerfile.dockerignore
# keeps everything else (SQLite files, .env, node_modules, target/) out of the
# build context.

# ── 1. React client (Node 20, as CI and Railway use) ────────────────────────
FROM node:20-bookworm-slim AS client
WORKDIR /app/client
COPY client/package.json client/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY client/ ./
RUN npm run build

# ── 2. Rust server ──────────────────────────────────────────────────────────
FROM rust:1-bookworm AS server
WORKDIR /app/server-rs
# SQL is checked at compile time against the committed .sqlx cache.
ENV SQLX_OFFLINE=true
# Build the dependencies first so code-only changes reuse this layer.
COPY server-rs/Cargo.toml server-rs/Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && touch src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src
COPY server-rs/ ./
# Make sure cargo sees the real sources as newer than the stub build.
RUN touch src/main.rs src/lib.rs \
    && cargo build --release --locked \
    && cp target/release/chef-server /usr/local/bin/chef-server

# ── 3. Runtime ──────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# AUTO_BACKUP=true (a Railway service variable is also passed as a build arg)
# installs pg_dump from the PostgreSQL apt repository, matching PG_MAJOR to the
# server's major version (pg_dump refuses to dump a newer server). Off by
# default: Railway's managed Postgres takes its own backups.
ARG AUTO_BACKUP=false
ARG PG_MAJOR=17

RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates; \
    if [ "$AUTO_BACKUP" = "true" ]; then \
        apt-get install -y --no-install-recommends postgresql-common; \
        /usr/share/postgresql-common/pgdg/apt.postgresql.org.sh -y; \
        apt-get install -y --no-install-recommends "postgresql-client-${PG_MAJOR}"; \
    fi; \
    rm -rf /var/lib/apt/lists/*; \
    useradd --uid 10001 --home-dir /app --no-create-home --shell /usr/sbin/nologin chef

WORKDIR /app
COPY --from=server /usr/local/bin/chef-server /usr/local/bin/chef-server
COPY --from=client /app/client/dist /app/client/dist
RUN mkdir -p /app/backups && chown chef /app/backups

ENV NODE_ENV=production \
    CLIENT_DIST=/app/client/dist \
    BACKUP_DIR=/app/backups \
    PORT=3000 \
    RUST_LOG=info,tower_http=info

USER chef
EXPOSE 3000
CMD ["chef-server"]
