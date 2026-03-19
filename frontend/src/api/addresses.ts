import client from './client';
import type { Address, PaginatedResponse, AddressTransfer } from '../types';

export async function getAddress(address: string): Promise<Address> {
  return client.get<Address>(`/addresses/${address}`);
}

export interface GetAddressesParams {
  page?: number;
  limit?: number;
  address_type?: 'eoa' | 'contract' | 'erc20' | 'nft';
  from_block?: number;
  to_block?: number;
}

export async function getAddresses(params: GetAddressesParams = {}): Promise<PaginatedResponse<Address>> {
  return client.get<PaginatedResponse<Address>>('/addresses', { params: params as Record<string, unknown> });
}

// Etherscan-compatible endpoint for native balance: GET /api?module=account&action=balance&address=...
interface EtherscanLikeResponse<T> { result: T }

export async function getEthBalance(address: string): Promise<string> {
  const data = await client.get<EtherscanLikeResponse<string>>('', {
    params: { module: 'account', action: 'balance', address },
  });
  return data.result ?? '0';
}

export interface GetAddressTransfersParams {
  page?: number;
  limit?: number;
  transfer_type?: 'erc20' | 'nft';
}

export async function getAddressTransfers(address: string, params: GetAddressTransfersParams = {}): Promise<PaginatedResponse<AddressTransfer>> {
  return client.get<PaginatedResponse<AddressTransfer>>(`/addresses/${address}/transfers`, { params: params as Record<string, unknown> });
}
