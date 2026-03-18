import axios from 'axios';
import type { AxiosInstance, AxiosError } from 'axios';
import type { ApiError } from '../types';

export const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || 'http://localhost:3000/api';

const client: AxiosInstance = axios.create({
  baseURL: API_BASE_URL,
  timeout: 10000,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Response interceptor for error handling
client.interceptors.response.use(
  (response) => response,
  (error: AxiosError<ApiError>) => {
    if (error.response) {
      const data = error.response.data as Partial<ApiError> | undefined;
      const retryAfterSeconds = parseRetryAfterSeconds(error.response.headers, data);
      const apiError: ApiError = {
        error: data?.error || error.message,
        status: error.response.status,
        ...(retryAfterSeconds !== undefined ? { retryAfterSeconds } : {}),
      };
      return Promise.reject(apiError);
    }

    if (error.request) {
      const networkError: ApiError = {
        error: 'Unable to connect to the API server',
      };
      return Promise.reject(networkError);
    }

    return Promise.reject({
      error: error.message,
    } as ApiError);
  }
);

function parseRetryAfterSeconds(
  headers: unknown,
  data: Partial<ApiError> | undefined
): number | undefined {
  const bodyRetryAfter = (data as { retry_after_seconds?: unknown } | undefined)?.retry_after_seconds;
  const headerRetryAfter = getHeaderValue(headers, 'retry-after');
  const rawRetryAfter = bodyRetryAfter ?? headerRetryAfter;

  if (typeof rawRetryAfter === 'number' && Number.isFinite(rawRetryAfter) && rawRetryAfter >= 0) {
    return rawRetryAfter;
  }

  if (typeof rawRetryAfter === 'string') {
    const parsed = Number(rawRetryAfter);
    if (Number.isFinite(parsed) && parsed >= 0) {
      return parsed;
    }

    const retryDate = Date.parse(rawRetryAfter);
    if (!Number.isNaN(retryDate)) {
      return Math.max(0, Math.ceil((retryDate - Date.now()) / 1000));
    }
  }

  return undefined;
}

function getHeaderValue(headers: unknown, key: string): unknown {
  if (!headers || typeof headers !== 'object') return undefined;
  const headerObject = headers as { get?: (name: string) => unknown } & Record<string, unknown>;

  if (typeof headerObject.get === 'function') {
    return headerObject.get(key);
  }

  return headerObject[key] ?? headerObject[key.toLowerCase()] ?? headerObject[key.toUpperCase()];
}

export default client;
