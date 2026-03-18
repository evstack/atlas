import type { ApiError } from '../types';

export function toApiError(error: unknown, fallback: string): ApiError {
  if (typeof error === 'object' && error !== null && 'error' in error && typeof (error as { error?: unknown }).error === 'string') {
    const apiError = error as ApiError;
    return {
      error: apiError.error,
      ...(typeof apiError.status === 'number' ? { status: apiError.status } : {}),
      ...(typeof apiError.retryAfterSeconds === 'number' ? { retryAfterSeconds: apiError.retryAfterSeconds } : {}),
    };
  }

  return {
    error: error instanceof Error ? error.message : fallback,
  };
}
