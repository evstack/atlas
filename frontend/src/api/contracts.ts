import { API_BASE_URL } from './client';
import type { ContractDetail, VerifyContractRequest, AbiItem } from '../types';

export async function getContractDetail(address: string): Promise<ContractDetail> {
  const res = await fetch(`${API_BASE_URL}/contracts/${address}`);
  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw { error: data.error ?? res.statusText, status: res.status };
  }
  return res.json();
}

export interface VerifyContractResponse {
  verified: boolean;
  abi: AbiItem[];
}

export async function verifyContract(
  address: string,
  req: VerifyContractRequest
): Promise<VerifyContractResponse> {
  // Use a 120s timeout — solc download + compilation can take a while
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 120_000);

  let res: Response;
  try {
    res = await fetch(`${API_BASE_URL}/contracts/${address}/verify`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(req),
      signal: controller.signal,
    });
  } catch (err) {
    clearTimeout(timer);
    if (err instanceof DOMException && err.name === 'AbortError') {
      throw { error: 'Verification timed out (compilation took too long)' };
    }
    throw { error: 'Unable to connect to the API server' };
  }

  clearTimeout(timer);

  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw { error: data.error ?? res.statusText, status: res.status };
  }

  return res.json();
}
