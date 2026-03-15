import { createContext } from 'react';
import type { NewBlockEvent, DaUpdateEvent, DaSubscriber } from '../hooks/useBlockSSE';

export interface BlockStats {
  bps: number | null;
  height: number | null;
  latestBlockEvent: NewBlockEvent | null;
  latestDaUpdate: DaUpdateEvent | null;
  sseConnected: boolean;
  subscribeDa: (cb: DaSubscriber) => () => void;
}

const noopSubscribe = () => () => {};

export const BlockStatsContext = createContext<BlockStats>({
  bps: null,
  height: null,
  latestBlockEvent: null,
  latestDaUpdate: null,
  sseConnected: false,
  subscribeDa: noopSubscribe,
});
