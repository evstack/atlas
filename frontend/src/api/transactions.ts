import client from './client';
import type { Transaction, PaginatedResponse, TxErc20Transfer, TxNftTransfer } from '../types';

export interface GetTransactionsParams {
  page?: number;
  limit?: number;
  block_number?: number;
  address?: string;
}

export async function getTransactions(params: GetTransactionsParams = {}): Promise<PaginatedResponse<Transaction>> {
  const { page = 1, limit = 20, block_number, address } = params;
  const response = await client.get<PaginatedResponse<Transaction>>('/transactions', {
    params: { page, limit, block_number, address },
  });
  return response.data;
}

export async function getTransactionByHash(txHash: string): Promise<Transaction> {
  const response = await client.get<Transaction>(`/transactions/${txHash}`);
  return response.data;
}

export async function getTransactionsByBlock(blockNumber: number, params: { page?: number; limit?: number } = {}): Promise<PaginatedResponse<Transaction>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<Transaction>>(`/blocks/${blockNumber}/transactions`, {
    params: { page, limit },
  });
  return response.data;
}

export async function getTransactionsByAddress(address: string, params: { page?: number; limit?: number } = {}): Promise<PaginatedResponse<Transaction>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<Transaction>>(`/addresses/${address}/transactions`, {
    params: { page, limit },
  });
  return response.data;
}

export async function getTxErc20Transfers(txHash: string): Promise<TxErc20Transfer[]> {
  const response = await client.get(`/transactions/${txHash}/erc20-transfers`);
  const body = response.data as any;
  if (Array.isArray(body)) return body as TxErc20Transfer[];
  if (Array.isArray(body?.data)) return body.data as TxErc20Transfer[];
  return [];
}

export async function getTxNftTransfers(txHash: string): Promise<TxNftTransfer[]> {
  const response = await client.get(`/transactions/${txHash}/nft-transfers`);
  const body = response.data as any;
  if (Array.isArray(body)) return body as TxNftTransfer[];
  if (Array.isArray(body?.data)) return body.data as TxNftTransfer[];
  return [];
}
