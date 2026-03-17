set shell := ["bash", "-cu"]

default:
  @just --list

# Frontend
frontend-install:
  cd frontend && bun install --frozen-lockfile

frontend-dev:
  cd frontend && bun run dev

frontend-lint:
  cd frontend && bun run lint

frontend-build:
  cd frontend && bun run build

# Backend
backend-fmt:
  cd backend && cargo fmt --all --check

backend-clippy:
  cd backend && cargo clippy --workspace --all-targets -- -D warnings

backend-test:
  cd backend && cargo test --workspace --all-targets

backend-server:
  cd backend && cargo run --bin atlas-server

# Docker
rpc_url := "https://ev-reth-eden-testnet.binarybuilders.services:8545"

# Run full stack against eden testnet, starting from latest block
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

# Combined checks
ci: backend-fmt backend-clippy backend-test frontend-install frontend-lint frontend-build
