# Compute a dependency recipe from the workspace manifests and lock file
FROM lukemathwalker/cargo-chef:latest-rust-1.85-alpine AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Build stage — cook dependencies first, then compile app code
FROM lukemathwalker/cargo-chef:latest-rust-1.85-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /app

# Cook dependencies (cached until Cargo.toml/Cargo.lock change)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source and build (only this layer rebuilds on code changes)
COPY . .

# Downgrade home crate to version compatible with Rust 1.85
RUN cargo update home@0.5.12 --precise 0.5.9 || true
RUN cargo update wit-bindgen --precise 0.41.0 || true
RUN cargo update wasip2 --precise 0.1.0 || true

RUN cargo build --release

# Indexer image
FROM alpine:3.21 AS indexer

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/atlas-indexer /usr/local/bin/

CMD ["atlas-indexer"]

# API image
FROM alpine:3.21 AS api

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/atlas-api /usr/local/bin/

EXPOSE 3000
CMD ["atlas-api"]
