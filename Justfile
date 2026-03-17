set shell := ["bash", "-cu"]

default:
  @just --list

# Run all quality checks (format, lint, typecheck, test, build)
[group('quality')]
quality: check test build

# Fast static checks only (no compilation, no tests)
[group('quality')]
check: fmt lint frontend-lint frontend-typecheck

# All tests
[group('quality')]
test: backend-test

# All builds (proves compilation + artifacts)
[group('quality')]
build: backend-build frontend-build

# Full CI pipeline
[group('quality')]
ci: quality

# Check Rust formatting
[group('backend')]
fmt:
  cd backend && cargo fmt --all --check

# Fix Rust formatting in-place
[group('backend')]
fmt-fix:
  cd backend && cargo fmt --all

# Run clippy
[group('backend')]
lint: backend-clippy

[group('backend')]
backend-clippy:
  cd backend && cargo clippy --workspace --all-targets -- -D warnings

[group('backend')]
backend-test:
  cd backend && cargo test --workspace --all-targets

[group('backend')]
backend-build:
  cd backend && cargo build --workspace

[group('backend')]
backend-run:
  cd backend && cargo run --bin atlas-server

[group('frontend')]
frontend-install:
  cd frontend && bun install --frozen-lockfile

[group('frontend')]
frontend-dev:
  cd frontend && bun run dev

[group('frontend')]
frontend-lint: frontend-install
  cd frontend && bun run lint

[group('frontend')]
frontend-typecheck: frontend-install
  cd frontend && bunx tsc -b --noEmit

[group('frontend')]
frontend-build: frontend-install
  cd frontend && bun run build

[group('docker')]
docker-up:
  docker compose up -d

[group('docker')]
docker-build:
  docker compose build

[group('docker')]
docker-down:
  docker compose down

[group('docker')]
docker-logs service="atlas-server":
  docker compose logs -f {{service}}

[group('docker')]
docker-rebuild service="atlas-server":
  docker compose build {{service}} && docker compose up -d {{service}}

# Docker
rpc_url := "https://ev-reth-eden-testnet.binarybuilders.services:8545"

# Run full stack against eden testnet, starting from latest block
[group('docker')]
test-run:
  #!/usr/bin/env bash
  latest=$(curl -s -X POST {{rpc_url}} \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
    | jq -r '.result')
  start_block=$((latest))
  echo "Latest block: $start_block"
  RPC_URL={{rpc_url}} \
  START_BLOCK=$start_block \
  REINDEX=false \
  RPC_REQUESTS_PER_SECOND=10000 \
  FETCH_WORKERS=10 \
  BATCH_SIZE=10000 \
  RPC_BATCH_SIZE=100 \
    docker compose up --build
