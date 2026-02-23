    # Architecture

## Overview

Atlas is a modular Ethereum L2 blockchain indexer and API server built in Rust. It indexes blocks, transactions, ERC-20 tokens, and NFTs from any EVM-compatible chain.

## System Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    PostgreSQL Database                       │
│  (Partitioned tables for blocks, transactions, transfers)   │
└─────────────────────────────────────────────────────────────┘
         ↑                                           ↑
         │ (Read-Write)                              │ (Read-Only)
    ┌────────────────────┐              ┌────────────────────────┐
    │  Atlas Indexer     │              │     Atlas API Server   │
    │  ───────────────   │              │  ────────────────────  │
    │ • Block Fetcher    │              │ • REST Endpoints       │
    │ • TX Processing    │              │ • Contract ABIs        │
    │ • Event Parsing    │              │ • Etherscan Compat     │
    │ • Metadata Fetcher │              │ • Search               │
    └────────────────────┘              └────────────────────────┘
              │
              ↓
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
│   │   ├── atlas-indexer/    # Block indexer + metadata fetcher
│   │   └── atlas-api/        # REST API server (Axum)
│   └── migrations/           # PostgreSQL migrations
├── frontend/                 # React frontend (Vite + Tailwind)
└── docker-compose.yml
```
