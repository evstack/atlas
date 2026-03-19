import client from './client';
import type { NftContract, NftToken, NftTransfer, PaginatedResponse } from '../types';

export interface GetNftContractsParams {
  page?: number;
  limit?: number;
}

export async function getNftContracts(params: GetNftContractsParams = {}): Promise<PaginatedResponse<NftContract>> {
  const { page = 1, limit = 20 } = params;
  return client.get<PaginatedResponse<NftContract>>('/nfts/collections', { params: { page, limit } });
}

export async function getNftContract(contractAddress: string): Promise<NftContract> {
  return client.get<NftContract>(`/nfts/collections/${contractAddress}`);
}

export interface GetNftTokensParams {
  page?: number;
  limit?: number;
  owner?: string;
}

export async function getNftTokens(contractAddress: string, params: GetNftTokensParams = {}): Promise<PaginatedResponse<NftToken>> {
  const { page = 1, limit = 20, owner } = params;
  return client.get<PaginatedResponse<NftToken>>(`/nfts/collections/${contractAddress}/tokens`, {
    params: { page, limit, owner },
  });
}

export async function getNftToken(contractAddress: string, tokenId: string): Promise<NftToken> {
  return client.get<NftToken>(`/nfts/collections/${contractAddress}/tokens/${tokenId}`);
}

export interface GetNftTransfersParams {
  page?: number;
  limit?: number;
}

export async function getNftTransfers(contractAddress: string, params: GetNftTransfersParams = {}): Promise<PaginatedResponse<NftTransfer>> {
  const { page = 1, limit = 20 } = params;
  return client.get<PaginatedResponse<NftTransfer>>(`/nfts/collections/${contractAddress}/transfers`, {
    params: { page, limit },
  });
}

export async function getNftTokenTransfers(contractAddress: string, tokenId: string, params: GetNftTransfersParams = {}): Promise<PaginatedResponse<NftTransfer>> {
  const { page = 1, limit = 20 } = params;
  return client.get<PaginatedResponse<NftTransfer>>(`/nfts/collections/${contractAddress}/tokens/${tokenId}/transfers`, {
    params: { page, limit },
  });
}

export interface GetAddressNftsParams {
  page?: number;
  limit?: number;
}

export async function getAddressNfts(address: string, params: GetAddressNftsParams = {}): Promise<PaginatedResponse<NftToken>> {
  const { page = 1, limit = 20 } = params;
  return client.get<PaginatedResponse<NftToken>>(`/addresses/${address}/nfts`, {
    params: { page, limit },
  });
}
