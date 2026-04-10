# Build stage
FROM rust:1.91-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /app
COPY . .

RUN cargo build --release

# Server image
FROM alpine:3.21 AS server

RUN apk add --no-cache ca-certificates gcompat postgresql16-client

COPY --from=builder /app/target/release/atlas-server /usr/local/bin/

RUN addgroup -S -g 1000 atlas && adduser -S -u 1000 atlas -G atlas \
    && mkdir -p /snapshots \
    && chown atlas:atlas /snapshots
USER atlas

EXPOSE 3000
CMD ["atlas-server", "run"]

# Backward-compatible target names for CI jobs that still build the old images.
FROM server AS api

FROM server AS indexer
