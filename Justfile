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

backend-api:
  cd backend && cargo run --bin atlas-api

backend-indexer:
  cd backend && cargo run --bin atlas-indexer

# Combined checks
ci: backend-fmt backend-clippy backend-test frontend-lint frontend-build
