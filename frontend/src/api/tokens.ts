import client from './client';
import type { Token, TokenHolder, TokenTransfer, AddressTokenBalance, PaginatedResponse } from '../types';

export interface GetTokensParams {
  page?: number;
  limit?: number;
}

export async function getTokens(params: GetTokensParams = {}): Promise<PaginatedResponse<Token>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<Token>>('/tokens', {
    params: { page, limit },
  });
  return response.data;
}

export async function getToken(address: string): Promise<Token> {
  const response = await client.get<Token>(`/tokens/${address}`);
  return response.data;
}

export interface GetTokenHoldersParams {
  page?: number;
  limit?: number;
}

export async function getTokenHolders(address: string, params: GetTokenHoldersParams = {}): Promise<PaginatedResponse<TokenHolder>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<TokenHolder>>(`/tokens/${address}/holders`, {
    params: { page, limit },
  });
  return response.data;
}

export interface GetTokenTransfersParams {
  page?: number;
  limit?: number;
}

export async function getTokenTransfers(address: string, params: GetTokenTransfersParams = {}): Promise<PaginatedResponse<TokenTransfer>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<TokenTransfer>>(`/tokens/${address}/transfers`, {
    params: { page, limit },
  });
  return response.data;
}

export interface GetAddressTokensParams {
  page?: number;
  limit?: number;
}

export async function getAddressTokens(address: string, params: GetAddressTokensParams = {}): Promise<PaginatedResponse<AddressTokenBalance>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<AddressTokenBalance>>(`/addresses/${address}/tokens`, {
    params: { page, limit },
  });
  return response.data;
}
