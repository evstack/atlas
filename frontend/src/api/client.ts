import type { ApiError } from '../types';

export const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || 'http://localhost:3000/api';

const TIMEOUT_MS = 10_000;

function buildUrl(path: string, params?: Record<string, unknown>): string {
  const base = API_BASE_URL.endsWith('/') ? API_BASE_URL.slice(0, -1) : API_BASE_URL;
  let url = base + path;

  if (params) {
    const qs = Object.entries(params)
      .filter(([, v]) => v !== undefined && v !== null)
      .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`)
      .join('&');
    if (qs) url += '?' + qs;
  }

  return url;
}

async function request<T>(
  method: 'GET' | 'POST',
  path: string,
  options: { params?: Record<string, unknown>; body?: unknown } = {}
): Promise<T> {
  const url = buildUrl(path, options.params);
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);

  let response: Response;
  try {
    response = await fetch(url, {
      method,
      headers: options.body !== undefined ? { 'Content-Type': 'application/json' } : undefined,
      body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
      signal: controller.signal,
    });
  } catch (err) {
    clearTimeout(timer);
    if (err instanceof DOMException && err.name === 'AbortError') {
      throw { error: 'Request timed out' } as ApiError;
    }
    throw { error: 'Unable to connect to the API server' } as ApiError;
  }

  clearTimeout(timer);

  if (!response.ok) {
    let data: { error?: string; retry_after_seconds?: unknown } = {};
    try {
      data = await response.json();
    } catch { /* ignore */ }

    const retryAfterSeconds = parseRetryAfterSeconds(
      response.headers.get('retry-after'),
      data.retry_after_seconds
    );

    throw {
      error: data.error ?? response.statusText,
      status: response.status,
      ...(retryAfterSeconds !== undefined ? { retryAfterSeconds } : {}),
    } as ApiError;
  }

  return response.json() as Promise<T>;
}

function parseRetryAfterSeconds(header: string | null, body: unknown): number | undefined {
  const raw = body ?? header;

  if (typeof raw === 'number' && Number.isFinite(raw) && raw >= 0) return raw;

  if (typeof raw === 'string') {
    const parsed = Number(raw);
    if (Number.isFinite(parsed) && parsed >= 0) return parsed;

    const retryDate = Date.parse(raw);
    if (!Number.isNaN(retryDate)) return Math.max(0, Math.ceil((retryDate - Date.now()) / 1000));
  }

  return undefined;
}

const client = {
  get<T>(path: string, options: { params?: Record<string, unknown> } = {}): Promise<T> {
    return request<T>('GET', path, { params: options.params });
  },
  post<T>(path: string, body?: unknown): Promise<T> {
    return request<T>('POST', path, { body });
  },
};

export default client;
