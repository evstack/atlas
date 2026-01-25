import client from './client';
import type { AddressLabel, PaginatedResponse } from '../types';

export interface GetLabelsParams {
  page?: number;
  limit?: number;
  tag?: string;
}

export async function getLabels(params: GetLabelsParams = {}): Promise<PaginatedResponse<AddressLabel>> {
  const { page = 1, limit = 20, tag } = params;
  const response = await client.get<PaginatedResponse<AddressLabel>>('/labels', {
    params: { page, limit, tag },
  });
  return response.data;
}

export async function getLabel(address: string): Promise<AddressLabel> {
  const response = await client.get<AddressLabel>(`/labels/${address}`);
  return response.data;
}

export async function getLabelTags(): Promise<string[]> {
  const response = await client.get<string[]>('/labels/tags');
  return response.data;
}
