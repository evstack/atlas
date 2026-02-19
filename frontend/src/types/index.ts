// Block types - matches backend atlas-common types
export interface Block {
  number: number;
  hash: string;
  parent_hash: string;
  timestamp: number;
  gas_used: number;
  gas_limit: number;
  transaction_count: number;
  indexed_at: string;
}

// Transaction types
export interface Transaction {
  hash: string;
  block_number: number;
  block_index: number;
  from_address: string;
  to_address: string | null;
  value: string;
  gas_price: string;
  gas_used: number;
  input_data: string;
  status: boolean;
  contract_created: string | null;
  timestamp: number;
}

// Address types
export interface Address {
  address: string;
  first_seen_block: number;
  tx_count: number;
  // New fields from updated API
  address_type?: 'eoa' | 'contract' | 'nft' | 'erc20';
  name?: string | null;
  symbol?: string | null;
  total_supply?: string | null;
  decimals?: number; // for erc20
}

// NFT types
export interface NftContract {
  address: string;
  name: string | null;
  symbol: string | null;
  total_supply: number | null;
  first_seen_block: number;
}

export interface NftToken {
  contract_address: string;
  token_id: string;
  owner: string;
  token_uri: string | null;
  metadata_fetched: boolean;
  metadata: NftMetadata | null;
  image_url: string | null;
  name: string | null;
  last_transfer_block: number;
}

export interface NftMetadata {
  name?: string;
  description?: string;
  image?: string;
  attributes?: NftAttribute[];
  [key: string]: unknown;
}

export interface NftAttribute {
  trait_type: string;
  value: string | number;
  display_type?: string;
}

export interface NftTransfer {
  id: number;
  tx_hash: string;
  log_index: number;
  contract_address: string;
  token_id: string;
  from_address: string;
  to_address: string;
  block_number: number;
  timestamp: number;
}

// API response types
export interface PaginatedResponse<T> {
  data: T[];
  page: number;
  limit: number;
  total: number;
  total_pages: number;
}

export interface SearchResult {
  type: 'block' | 'transaction' | 'address' | 'nft' | 'nft_collection';
}

export interface BlockSearchResult extends SearchResult {
  type: 'block';
  number: number;
  hash: string;
  parent_hash: string;
  timestamp: number;
  gas_used: number;
  gas_limit: number;
  transaction_count: number;
  indexed_at: string;
}

export interface TransactionSearchResult extends SearchResult {
  type: 'transaction';
  hash: string;
  block_number: number;
  block_index: number;
  from_address: string;
  to_address: string | null;
  value: string;
  gas_price: string;
  gas_used: number;
  input_data: string;
  status: boolean;
  contract_created: string | null;
  timestamp: number;
}

export interface AddressSearchResult extends SearchResult {
  type: 'address';
  address: string;
  is_contract: boolean;
  first_seen_block: number;
  tx_count: number;
}

export interface NftSearchResult extends SearchResult {
  type: 'nft';
  contract_address: string;
  token_id: string;
  owner: string;
  name: string | null;
  image_url: string | null;
}

export interface NftCollectionSearchResult extends SearchResult {
  type: 'nft_collection';
  address: string;
  name: string | null;
  symbol: string | null;
  image_url?: string | null;
}

export type AnySearchResult =
  | BlockSearchResult
  | TransactionSearchResult
  | AddressSearchResult
  | NftSearchResult
  | NftCollectionSearchResult;

export interface SearchResponse {
  results: AnySearchResult[];
  query: string;
}

export interface ApiError {
  error: string;
}

// ERC-20 Token types
export interface Token {
  address: string;
  name: string | null;
  symbol: string | null;
  decimals: number;
  total_supply: string | null;
  first_seen_block: number;
}

export interface TokenHolder {
  address: string;
  balance: string;
  percentage: number;
}

export interface TokenTransfer {
  id: number;
  tx_hash: string;
  log_index: number;
  contract_address: string;
  from_address: string;
  to_address: string;
  value: string;
  block_number: number;
  timestamp: number;
}

export interface AddressTokenBalance {
  contract_address: string;
  name: string | null;
  symbol: string | null;
  decimals: number;
  balance: string;
}

// Address-level combined transfers (ERC-20 + NFT)
export interface AddressTransfer {
  tx_hash: string;
  log_index: number;
  contract_address: string;
  from_address: string;
  to_address: string;
  value: string; // ERC-20 amount (raw) or NFT token ID
  block_number: number;
  timestamp: number;
  transfer_type: 'erc20' | 'nft';
  token_name: string | null;
  token_symbol: string | null;
}

// Transaction-level transfer types (from dedicated endpoints)
export interface TxErc20Transfer {
  tx_hash: string;
  contract_address: string;
  from_address: string;
  to_address: string;
  value: string;
}

export interface TxNftTransfer {
  tx_hash: string;
  contract_address: string;
  token_id: string;
  from_address: string;
  to_address: string;
}

// Event Log types
export interface EventLog {
  id: number;
  tx_hash: string;
  log_index: number;
  address: string;
  topic0: string;
  topic1: string | null;
  topic2: string | null;
  topic3: string | null;
  data: string;
  block_number: number;
  timestamp: number;
}

export interface DecodedEventLog extends EventLog {
  event_name: string | null;
  event_signature: string | null;
  decoded_params: DecodedParam[] | null;
}

export interface DecodedParam {
  name: string;
  type: string;
  value: string;
  indexed: boolean;
}


// Proxy Contract types
export interface ProxyInfo {
  proxy_address: string;
  implementation_address: string;
  proxy_type: string;
  detected_at_block: number;
  is_current: boolean;
}

export interface CombinedAbi {
  proxy_address: string;
  implementation_address: string;
  proxy_type: string;
  combined_abi: AbiItem[];
}

export interface AbiItem {
  type: string;
  name?: string;
  inputs?: AbiInput[];
  outputs?: AbiOutput[];
  stateMutability?: string;
  anonymous?: boolean;
}

export interface AbiInput {
  name: string;
  type: string;
  indexed?: boolean;
  components?: AbiInput[];
}

export interface AbiOutput {
  name: string;
  type: string;
  components?: AbiOutput[];
}
