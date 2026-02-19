import { createContext } from 'react';

export interface BlockStats {
  bps: number | null;
  height: number | null;
}

export const BlockStatsContext = createContext<BlockStats>({ bps: null, height: null });

