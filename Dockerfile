# =============================================================================
# Stage 1: Build Rust server
# =============================================================================
FROM rust:1.77-slim-bookworm AS builder-server

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY dmart-shared/ dmart-shared/
COPY dmart-server/ dmart-server/

RUN cargo build --release --package dmart-server

# =============================================================================
# Stage 2: Build WASM frontend
# =============================================================================
FROM rust:1.77-slim-bookworm AS builder-wasm

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

RUN cargo install trunk && rustup target add wasm32-unknown-unknown

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY dmart-shared/ dmart-shared/
COPY dmart-app/ dmart-app/
COPY dmart-app/Trunk.toml dmart-app/
COPY dmart-app/index.html dmart-app/
COPY dmart-app/input.css dmart-app/
COPY dmart-app/tailwind.config.js dmart-app/

RUN cd dmart-app && trunk build --release

# =============================================================================
# Stage 3: Runtime image
# =============================================================================
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

RUN addgroup --system dmart && adduser --system --ingroup dmart dmart

WORKDIR /app

COPY --from=builder-server /build/target/release/dmart-server /app/dmart-server
COPY --from=builder-wasm /build/dmart-app/dist /app/dist

RUN mkdir -p /app/data && chown -R dmart:dmart /app

USER dmart

EXPOSE 3000

ENV DMART_PORT=3000
ENV DMART_DB_PATH=/app/data/dmart.db
ENV DMART_DIST_PATH=/app/dist

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD wget -qO- http://localhost:3000/api/health | grep -q '"status":"healthy"' || exit 1

ENTRYPOINT ["/app/dmart-server"]
