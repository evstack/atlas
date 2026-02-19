import { getBlocks } from './blocks';
import { getTransactions } from './transactions';
import { getAddresses } from './addresses';

export interface Totals {
  blocksTotal: number;
  transactionsTotal: number;
  addressesTotal: number;
}

export async function getTotals(): Promise<Totals> {
  const [blocks, txs, addrs] = await Promise.all([
    getBlocks({ page: 1, limit: 1 }),
    getTransactions({ page: 1, limit: 1 }),
    getAddresses({ page: 1, limit: 1 }),
  ]);

  return {
    blocksTotal: blocks.total,
    transactionsTotal: txs.total,
    addressesTotal: addrs.total,
  };
}

export async function getDailyTxCount(maxPages = 50, pageLimit = 100): Promise<number> {
  // Count transactions in the last 24 hours by paginating recent transactions
  // Stops early once crossing the 24h threshold or max pages reached
  const nowSec = Math.floor(Date.now() / 1000);
  const cutoff = nowSec - 24 * 60 * 60;
  let page = 1;
  let count = 0;
  // Loop with a sane cap to avoid heavy queries on very busy chains
  while (page <= maxPages) {
    const res = await getTransactions({ page, limit: pageLimit });
    if (!res.data.length) break;
    for (const tx of res.data) {
      if (tx.timestamp >= cutoff) count += 1; else {
        // As transactions are sorted desc by block/time, we can stop here
        return count;
      }
    }
    // If last item is still newer than cutoff and we have more pages, continue
    if (res.page >= res.total_pages) break;
    page += 1;
  }
  return count;
}

