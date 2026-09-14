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
# Stage 2: Build WASM frontend (cargo + wasm-bindgen, bypass trunk)
# =============================================================================
FROM rust:1.98-slim-bookworm AS builder-wasm

RUN apt-get update && apt-get install -y pkg-config libssl-dev wget && rm -rf /var/lib/apt/lists/*

# Install wasm-bindgen-cli (match dmart-app's wasm-bindgen 0.2.128)
RUN cargo install wasm-bindgen-cli --version 0.2.128 --locked \
    && rustup target add wasm32-unknown-unknown

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY dmart-shared/ dmart-shared/
COPY dmart-app/ dmart-app/
COPY dmart-server/ dmart-server/
COPY dmart-app/index.html dmart-app/
COPY dmart-app/input.css dmart-app/
COPY dmart-app/manifest.webmanifest dmart-app/
COPY dmart-app/sw.js dmart-app/
COPY dmart-app/icon.svg dmart-app/

# Build WASM release
RUN cd dmart-app \
    && cargo build --target wasm32-unknown-unknown --release \
    && wasm-bindgen --out-dir ../dist --target web ../target/wasm32-unknown-unknown/release/dmart_app.wasm \
    && cp index.html ../dist/ \
    && cp input.css ../dist/ \
    && cp manifest.webmanifest ../dist/ \
    && cp sw.js ../dist/ \
    && cp icon.svg ../dist/

# =============================================================================
# Stage 3: Runtime image
# =============================================================================
FROM debian:bookworm-slim

# wget for HEALTHCHECK, sqlite3 for backup, gzip for compression
RUN apt-get update && apt-get install -y ca-certificates wget sqlite3 gzip && rm -rf /var/lib/apt/lists/*

RUN addgroup --system dmart && adduser --system --ingroup dmart dmart

WORKDIR /app

COPY --from=builder-server /build/target/release/dmart-server /app/dmart-server
COPY --from=builder-wasm /build/dist /app/dist
COPY dmart-server/scripts/backup.sh /app/scripts/backup.sh

RUN mkdir -p /app/data /app/scripts && chmod +x /app/scripts/backup.sh && chown -R dmart:dmart /app

USER dmart

EXPOSE 3000

ENV DMART_PORT=3000
ENV DMART_DB_PATH=/app/data/dmart.db
ENV DMART_DIST_PATH=/app/dist

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD wget -qO- http://localhost:3000/obs/health | grep -q '"status":"healthy"' || exit 1

ENTRYPOINT ["/app/dmart-server"]