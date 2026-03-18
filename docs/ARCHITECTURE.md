# Architecture

## Overview

Atlas is a modular Ethereum L2 blockchain indexer and API server built in Rust. The `atlas-server` process runs both the indexer and the HTTP API, indexing blocks, transactions, ERC-20 tokens, and NFTs from any EVM-compatible chain.

## System Diagram

```
┌────────────────────────────────────────────────────────────────────┐
│                         atlas-server process                        │
│                                                                    │
│  ┌────────────────────┐      post-commit publish      ┌──────────┐ │
│  │      Indexer       │ ────────────────────────────► │HeadTracker│ │
│  │ • RPC block fetch  │                               │ latest    │ │
│  │ • Batch assembly   │                               │ live tail │ │
│  │ • DB writes        │                               └─────┬────┘ │
│  └─────────┬──────────┘                                     │      │
│            │                                                │      │
│            ▼                                                ▼      │
│  ┌────────────────────┐                           ┌────────────────┐│
│  │ PostgreSQL         │                           │ HTTP API       ││
│  │ canonical history  │                           │ • REST         ││
│  │ blocks/indexes     │                           │ • SSE events   ││
│  └────────────────────┘                           └────────────────┘│
└────────────────────────────────────────────────────────────────────┘
                             ▲
                             │
                    ┌─────────────────────┐
                    │  Ethereum Node      │
                    │  (JSON-RPC)         │
                    └─────────────────────┘
```

## Project Structure

```
atlas/
├── backend/
│   ├── crates/
│   │   ├── atlas-common/     # Shared types, DB models, error handling
│   │   └── atlas-server/     # Combined indexer + API server (Axum)
│   └── migrations/           # PostgreSQL migrations
├── frontend/                 # React frontend (Vite + Tailwind)
└── docker-compose.yml
```
