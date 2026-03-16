# Build stage
FROM rust:1.85-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /app
COPY . .

# Downgrade home crate to version compatible with Rust 1.85
RUN cargo update home@0.5.12 --precise 0.5.9 || true
RUN cargo update wit-bindgen --precise 0.41.0 || true
RUN cargo update wasip2 --precise 0.1.0 || true

RUN cargo build --release

# Server image
FROM alpine:3.21 AS server

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/atlas-server /usr/local/bin/

RUN addgroup -S atlas && adduser -S atlas -G atlas
USER atlas

EXPOSE 3000
CMD ["atlas-server"]

# Backward-compatible target names for CI jobs that still build the old images.
FROM server AS api

FROM server AS indexer
