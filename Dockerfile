# MultiParity server image: the API, the web UI and the `mp` command line in one container.
#
#   docker build -t multiparity .
#
# Stages: the web UI (Node), the Rust binaries, then a small Debian runtime.

FROM node:22-bookworm-slim AS web
WORKDIR /web
RUN corepack enable && corepack prepare pnpm@9.15.0 --activate
COPY web/package.json web/pnpm-lock.yaml* ./
RUN pnpm install --frozen-lockfile || pnpm install
COPY web/ ./
# The theme list and engine are shared with the desktop app.
COPY desktop/ui/theme-engine.js desktop/ui/themes.json /desktop/ui/
RUN pnpm build

FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release -p mp-server -p mp-cli && \
    mkdir -p /out && cp target/release/mp-server target/release/mp /out/

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl tini && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home /data multiparity && mkdir -p /data/blobs /data/backups /data/desktop /data/exports && chown -R multiparity /data
COPY --from=build /out/mp-server /out/mp /usr/local/bin/
COPY --from=web /web/dist /app/web
ENV MP_LISTEN=0.0.0.0:18500 \
    MP_WEB_DIR=/app/web \
    MP_STORAGE=local \
    MP_STORAGE_PATH=/data/blobs \
    RUST_LOG=info,tower_http=warn,sqlx=warn
USER multiparity
VOLUME ["/data"]
EXPOSE 18500
HEALTHCHECK --interval=15s --timeout=5s --start-period=20s CMD curl -fs http://127.0.0.1:18500/api/health || exit 1
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["mp-server"]
