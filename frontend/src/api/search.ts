import client from './client';
import type { SearchResponse } from '../types';

export async function search(query: string): Promise<SearchResponse> {
  return client.get<SearchResponse>('/search', { params: { q: query } });
}
