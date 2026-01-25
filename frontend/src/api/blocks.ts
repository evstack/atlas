import client from './client';
import type { Block, Transaction, PaginatedResponse } from '../types';

export interface GetBlocksParams {
  page?: number;
  limit?: number;
}

export async function getBlocks(params: GetBlocksParams = {}): Promise<PaginatedResponse<Block>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<Block>>('/blocks', {
    params: { page, limit },
  });
  return response.data;
}

export async function getBlockByNumber(blockNumber: number): Promise<Block> {
  const response = await client.get<Block>(`/blocks/${blockNumber}`);
  return response.data;
}

export async function getBlockByHash(blockHash: string): Promise<Block> {
  const response = await client.get<Block>(`/blocks/hash/${blockHash}`);
  return response.data;
}

export async function getBlockTransactions(
  blockNumber: number,
  params: GetBlocksParams = {}
): Promise<PaginatedResponse<Transaction>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<Transaction>>(
    `/blocks/${blockNumber}/transactions`,
    { params: { page, limit } }
  );
  return response.data;
}
