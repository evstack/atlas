import client from './client';
import type { Address, PaginatedResponse } from '../types';
import type { AddressTransfer } from '../types';

export async function getAddress(address: string): Promise<Address> {
  const response = await client.get<Address>(`/addresses/${address}`);
  return response.data;
}

export interface GetAddressesParams {
  page?: number;
  limit?: number;
  // Updated to align with backend API
  address_type?: 'eoa' | 'contract' | 'erc20' | 'nft';
  from_block?: number;
  to_block?: number;
}

export async function getAddresses(params: GetAddressesParams = {}): Promise<PaginatedResponse<Address>> {
  const response = await client.get<PaginatedResponse<Address>>('/addresses', { params });
  return response.data;
}

// Etherscan-compatible endpoint for native balance: GET /api?module=account&action=balance&address=...
interface EtherscanLikeResponse<T> { result: T }

export async function getEthBalance(address: string): Promise<string> {
  const response = await client.get<EtherscanLikeResponse<string>>('', {
    params: { module: 'account', action: 'balance', address },
  });
  // Returns Wei as string
  return response.data.result ?? '0';
}

// New: Address transfers (ERC-20 + NFT)
export interface GetAddressTransfersParams {
  page?: number;
  limit?: number;
  transfer_type?: 'erc20' | 'nft';
}

export async function getAddressTransfers(address: string, params: GetAddressTransfersParams = {}): Promise<PaginatedResponse<AddressTransfer>> {
  const response = await client.get<PaginatedResponse<AddressTransfer>>(`/addresses/${address}/transfers`, { params });
  return response.data;
}
