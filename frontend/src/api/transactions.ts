import client from './client';
import type { Transaction, PaginatedResponse } from '../types';

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
