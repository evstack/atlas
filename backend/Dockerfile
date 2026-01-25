# Build stage
FROM rust:1.83-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /app
COPY . .

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
