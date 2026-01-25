import axios from 'axios';
import type { AxiosInstance, AxiosError } from 'axios';
import type { ApiError } from '../types';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || 'http://localhost:3000/api';

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
      const apiError: ApiError = {
        error: error.response.data?.error || error.message,
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

export default client;
