import client from './client';
import type { EventLog, DecodedEventLog, PaginatedResponse } from '../types';

export interface GetTransactionLogsParams {
  page?: number;
  limit?: number;
}

export async function getTransactionLogs(txHash: string, params: GetTransactionLogsParams = {}): Promise<PaginatedResponse<EventLog>> {
  const { page = 1, limit = 50 } = params;
  const response = await client.get<PaginatedResponse<EventLog>>(`/transactions/${txHash}/logs`, {
    params: { page, limit },
  });
  return response.data;
}

export async function getTransactionDecodedLogs(txHash: string, params: GetTransactionLogsParams = {}): Promise<PaginatedResponse<DecodedEventLog>> {
  const { page = 1, limit = 50 } = params;
  const response = await client.get<PaginatedResponse<DecodedEventLog>>(`/transactions/${txHash}/logs/decoded`, {
    params: { page, limit },
  });
  return response.data;
}

export interface GetAddressLogsParams {
  page?: number;
  limit?: number;
}

export async function getAddressLogs(address: string, params: GetAddressLogsParams = {}): Promise<PaginatedResponse<EventLog>> {
  const { page = 1, limit = 50 } = params;
  const response = await client.get<PaginatedResponse<EventLog>>(`/addresses/${address}/logs`, {
    params: { page, limit },
  });
  return response.data;
}

export interface GetLogsByTopicParams {
  page?: number;
  limit?: number;
}

export async function getLogsByTopic(topic0: string, params: GetLogsByTopicParams = {}): Promise<PaginatedResponse<EventLog>> {
  const { page = 1, limit = 50 } = params;
  const response = await client.get<PaginatedResponse<EventLog>>('/logs', {
    params: { topic0, page, limit },
  });
  return response.data;
}
