# =============================================================================
# Stage 1: Build Rust server
# =============================================================================
FROM rust:1.98-slim-bookworm AS builder-server

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY dmart-shared/ dmart-shared/
COPY dmart-server/ dmart-server/
COPY dmart-app/ dmart-app/

RUN cargo build --release --package dmart-server

# =============================================================================
# Stage 2: Build WASM frontend
# =============================================================================
FROM rust:1.98-slim-bookworm AS builder-wasm

RUN apt-get update && apt-get install -y pkg-config libssl-dev wget && rm -rf /var/lib/apt/lists/*

# `cargo install trunk` no compila con rustc 1.98 (quiebre cssparser/parcel_selectors).
# Se usa el binario oficial prebuilt de trunk-rs (el repo se trasladó de trunkrs a trunk-rs).
RUN wget -q https://github.com/trunk-rs/trunk/releases/download/v0.21.14/trunk-x86_64-unknown-linux-gnu.tar.gz -O /tmp/trunk.tar.gz \
    && tar xzf /tmp/trunk.tar.gz -C /usr/local/bin trunk \
    && chmod +x /usr/local/bin/trunk \
    && rm /tmp/trunk.tar.gz \
    && rustup target add wasm32-unknown-unknown

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY dmart-shared/ dmart-shared/
COPY dmart-app/ dmart-app/
COPY dmart-server/ dmart-server/
COPY dmart-app/Trunk.toml dmart-app/
COPY dmart-app/index.html dmart-app/
COPY dmart-app/input.css dmart-app/
COPY dmart-app/tailwind.config.js dmart-app/

RUN cd dmart-app && trunk build --release

# =============================================================================
# Stage 3: Runtime image
# =============================================================================
FROM debian:bookworm-slim

# wget es necesario para el HEALTHCHECK (bookworm-slim NO lo incluye — bug latente).
RUN apt-get update && apt-get install -y ca-certificates wget && rm -rf /var/lib/apt/lists/*

RUN addgroup --system dmart && adduser --system --ingroup dmart dmart

WORKDIR /app

COPY --from=builder-server /build/target/release/dmart-server /app/dmart-server
COPY --from=builder-wasm /build/dist /app/dist

RUN mkdir -p /app/data && chown -R dmart:dmart /app

USER dmart

EXPOSE 3000

ENV DMART_PORT=3000
ENV DMART_DB_PATH=/app/data/dmart.db
ENV DMART_DIST_PATH=/app/dist

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD wget -qO- http://localhost:3000/obs/health | grep -q '"status":"healthy"' || exit 1

ENTRYPOINT ["/app/dmart-server"]
