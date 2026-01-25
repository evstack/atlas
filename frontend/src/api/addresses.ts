import client from './client';
import type { Address } from '../types';

export async function getAddress(address: string): Promise<Address> {
  const response = await client.get<Address>(`/addresses/${address}`);
  return response.data;
}
