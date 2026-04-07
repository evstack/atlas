import { createContext } from 'react';
import type { NewBlockEvent, DaSubscriber, DaResyncSubscriber } from '../hooks/useBlockSSE';

export interface BlockStats {
  bps: number | null;
  height: number | null;
  latestBlockEvent: NewBlockEvent | null;
  sseConnected: boolean;
  subscribeDa: (cb: DaSubscriber) => () => void;
  subscribeDaResync: (cb: DaResyncSubscriber) => () => void;
}

const noopSubscribe = () => () => {};

export const BlockStatsContext = createContext<BlockStats>({
  bps: null,
  height: null,
  latestBlockEvent: null,
  sseConnected: false,
  subscribeDa: noopSubscribe,
  subscribeDaResync: noopSubscribe,
});
