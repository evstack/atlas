import client from './client';
import type { Transaction, PaginatedResponse, TxErc20Transfer, TxNftTransfer } from '../types';

export interface GetTransactionsParams {
  page?: number;
  limit?: number;
  block_number?: number;
  address?: string;
  before_block?: number;
  before_index?: number;
  after_block?: number;
  after_index?: number;
  last_page?: boolean;
}

export async function getTransactions(params: GetTransactionsParams = {}): Promise<PaginatedResponse<Transaction>> {
  const { page = 1, limit = 20, block_number, address, before_block, before_index, after_block, after_index, last_page } = params;
  return client.get<PaginatedResponse<Transaction>>('/transactions', {
    params: { page, limit, block_number, address, before_block, before_index, after_block, after_index, last_page: last_page || undefined },
  });
}

export async function getTransactionByHash(txHash: string): Promise<Transaction> {
  return client.get<Transaction>(`/transactions/${txHash}`);
}

export async function getTransactionsByBlock(blockNumber: number, params: { page?: number; limit?: number } = {}): Promise<PaginatedResponse<Transaction>> {
  const { page = 1, limit = 20 } = params;
  return client.get<PaginatedResponse<Transaction>>(`/blocks/${blockNumber}/transactions`, {
    params: { page, limit },
  });
}

export async function getTransactionsByAddress(address: string, params: { page?: number; limit?: number } = {}): Promise<PaginatedResponse<Transaction>> {
  const { page = 1, limit = 20 } = params;
  return client.get<PaginatedResponse<Transaction>>(`/addresses/${address}/transactions`, {
    params: { page, limit },
  });
}

export async function getTxErc20Transfers(txHash: string): Promise<TxErc20Transfer[]> {
  const body = await client.get<unknown>(`/transactions/${txHash}/erc20-transfers`);
  if (Array.isArray(body)) return body as TxErc20Transfer[];
  if (typeof body === 'object' && body !== null && 'data' in body) {
    const data = (body as { data?: unknown }).data;
    if (Array.isArray(data)) return data as TxErc20Transfer[];
  }
  return [];
}

export async function getTxNftTransfers(txHash: string): Promise<TxNftTransfer[]> {
  const body = await client.get<unknown>(`/transactions/${txHash}/nft-transfers`);
  if (Array.isArray(body)) return body as TxNftTransfer[];
  if (typeof body === 'object' && body !== null && 'data' in body) {
    const data = (body as { data?: unknown }).data;
    if (Array.isArray(data)) return data as TxNftTransfer[];
  }
  return [];
}
