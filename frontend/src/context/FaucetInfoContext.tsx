import { createContext } from 'react';
import type { UseFaucetInfoResult } from '../hooks/useFaucetInfo';

const noop = async () => {};

export const FaucetInfoContext = createContext<UseFaucetInfoResult>({
  faucetInfo: null,
  loading: false,
  error: null,
  notFound: false,
  refetch: noop,
});
