# Atlas Frontend

React frontend for the Atlas blockchain explorer.

## Tech Stack

- React 19
- TypeScript
- Vite
- Tailwind CSS
- Bun (package manager)

## Development

```bash
# Install dependencies
bun install

# Start dev server
bun run dev

# Build for production
bun run build

# Run linter
bun run lint

# Preview production build
bun run preview
```

## Project Structure

```
frontend/
├── src/
│   ├── api/           # API client functions
│   │   ├── blocks.ts
│   │   ├── transactions.ts
│   │   ├── addresses.ts
│   │   ├── nfts.ts
│   │   ├── tokens.ts
│   │   ├── logs.ts
│   │   ├── labels.ts
│   │   ├── proxies.ts
│   │   └── search.ts
│   ├── components/    # Reusable UI components
│   │   ├── Layout.tsx
│   │   ├── Pagination.tsx
│   │   ├── AddressLink.tsx
│   │   ├── TxHashLink.tsx
│   │   ├── BlockLink.tsx
│   │   ├── EventLogs.tsx
│   │   ├── LabeledAddress.tsx
│   │   └── ProxyBadge.tsx
│   ├── hooks/         # React hooks for data fetching
│   │   ├── useBlocks.ts
│   │   ├── useTransactions.ts
│   │   ├── useAddresses.ts
│   │   ├── useNFTs.ts
│   │   ├── useTokens.ts
│   │   ├── useLogs.ts
│   │   ├── useLabels.ts
│   │   └── useProxies.ts
│   ├── pages/         # Page components
│   │   ├── HomePage.tsx
│   │   ├── BlocksPage.tsx
│   │   ├── BlockDetailPage.tsx
│   │   ├── TransactionsPage.tsx
│   │   ├── TransactionDetailPage.tsx
│   │   ├── AddressPage.tsx
│   │   ├── NFTsPage.tsx
│   │   ├── NFTContractPage.tsx
│   │   ├── NFTTokenPage.tsx
│   │   ├── TokensPage.tsx
│   │   ├── TokenDetailPage.tsx
│   │   └── SearchPage.tsx
│   ├── types/         # TypeScript type definitions
│   │   └── index.ts
│   ├── utils/         # Utility functions
│   │   └── format.ts
│   ├── App.tsx        # Router configuration
│   └── main.tsx       # Entry point
├── public/            # Static assets
├── index.html
├── package.json
├── bun.lock
├── vite.config.ts
├── tailwind.config.js
└── tsconfig.json
```

## Environment Variables

Create a `.env` file for local development:

```env
VITE_API_URL=http://localhost:3000
```

## Features

- **Blocks**: Browse and search blocks
- **Transactions**: View transaction details with decoded event logs
- **Addresses**: Address pages with transaction history, token balances, NFTs
- **NFTs**: Browse ERC-721 collections and tokens with metadata
- **Tokens**: Browse ERC-20 tokens with holders and transfers
- **Labels**: Display curated address labels
- **Proxy Detection**: Show proxy contract indicators
- **Search**: Universal search across all entities
