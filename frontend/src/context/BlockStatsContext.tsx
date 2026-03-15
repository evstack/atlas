import { createContext } from 'react';
import type { NewBlockEvent } from '../hooks/useBlockSSE';

export interface BlockStats {
  bps: number | null;
  height: number | null;
  latestBlockEvent: NewBlockEvent | null;
  sseConnected: boolean;
}

export const BlockStatsContext = createContext<BlockStats>({
  bps: null,
  height: null,
  latestBlockEvent: null,
  sseConnected: false,
});

