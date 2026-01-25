import client from './client';
import type { NftContract, NftToken, NftTransfer, PaginatedResponse } from '../types';

export interface GetNftContractsParams {
  page?: number;
  limit?: number;
}

export async function getNftContracts(params: GetNftContractsParams = {}): Promise<PaginatedResponse<NftContract>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<NftContract>>('/nfts', {
    params: { page, limit },
  });
  return response.data;
}

export async function getNftContract(contractAddress: string): Promise<NftContract> {
  const response = await client.get<NftContract>(`/nfts/${contractAddress}`);
  return response.data;
}

export interface GetNftTokensParams {
  page?: number;
  limit?: number;
  owner?: string;
}

export async function getNftTokens(contractAddress: string, params: GetNftTokensParams = {}): Promise<PaginatedResponse<NftToken>> {
  const { page = 1, limit = 20, owner } = params;
  const response = await client.get<PaginatedResponse<NftToken>>(`/nfts/${contractAddress}/tokens`, {
    params: { page, limit, owner },
  });
  return response.data;
}

export async function getNftToken(contractAddress: string, tokenId: string): Promise<NftToken> {
  const response = await client.get<NftToken>(`/nfts/${contractAddress}/tokens/${tokenId}`);
  return response.data;
}

export interface GetNftTransfersParams {
  page?: number;
  limit?: number;
}

export async function getNftTransfers(contractAddress: string, params: GetNftTransfersParams = {}): Promise<PaginatedResponse<NftTransfer>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<NftTransfer>>(`/nfts/${contractAddress}/transfers`, {
    params: { page, limit },
  });
  return response.data;
}

export async function getNftTokenTransfers(contractAddress: string, tokenId: string, params: GetNftTransfersParams = {}): Promise<PaginatedResponse<NftTransfer>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<NftTransfer>>(`/nfts/${contractAddress}/tokens/${tokenId}/transfers`, {
    params: { page, limit },
  });
  return response.data;
}
