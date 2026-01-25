import { useState, useEffect, useCallback } from 'react';
import type { AddressLabel, ApiError } from '../types';
import { getLabels, getLabel, getLabelTags } from '../api/labels';
import type { GetLabelsParams } from '../api/labels';

interface UseLabelsResult {
  labels: AddressLabel[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useLabels(params: GetLabelsParams = {}): UseLabelsResult {
  const [labels, setLabels] = useState<AddressLabel[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchLabels = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getLabels(params);
      setLabels(response.data);
      setPagination({
        page: response.page,
        limit: response.limit,
        total: response.total,
        total_pages: response.total_pages,
      });
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [params.page, params.limit, params.tag]);

  useEffect(() => {
    fetchLabels();
  }, [fetchLabels]);

  return { labels, pagination, loading, error, refetch: fetchLabels };
}

interface UseLabelResult {
  label: AddressLabel | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useLabel(address: string | undefined): UseLabelResult {
  const [label, setLabel] = useState<AddressLabel | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchLabel = useCallback(async () => {
    if (!address) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getLabel(address);
      setLabel(response);
    } catch {
      // Label not found is not an error - just means no label exists
      setLabel(null);
    } finally {
      setLoading(false);
    }
  }, [address]);

  useEffect(() => {
    fetchLabel();
  }, [fetchLabel]);

  return { label, loading, error, refetch: fetchLabel };
}

interface UseLabelTagsResult {
  tags: string[];
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useLabelTags(): UseLabelTagsResult {
  const [tags, setTags] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTags = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getLabelTags();
      setTags(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchTags();
  }, [fetchTags]);

  return { tags, loading, error, refetch: fetchTags };
}
