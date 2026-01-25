import client from './client';
import type { SearchResponse } from '../types';

export async function search(query: string): Promise<SearchResponse> {
  const response = await client.get<SearchResponse>('/search', {
    params: { q: query },
  });
  return response.data;
}
