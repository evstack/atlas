import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTransactions } from '../hooks';
import { AddressLink, BlockLink, StatusBadge, Loading } from '../components';
import { formatNumber, formatTimeAgo, formatEtherExact, truncateHash, formatUsd } from '../utils';
import { useEthPrice } from '../hooks';

export default function TransactionsPage() {
  const [page, setPage] = useState(1);
  const [autoRefresh, setAutoRefresh] = useState<boolean>(() => {
    try {
      const v = localStorage.getItem('txs:autoRefresh');
      return v === null ? true : v === 'true';
    } catch {
      return true;
    }
  });
  const [tick, setTick] = useState(0);
  const { transactions, pagination, refetch, loading } = useTransactions({ page, limit: 20 });
  const [hasLoaded, setHasLoaded] = useState(false);
  useEffect(() => {
    if (!loading) setHasLoaded(true);
  }, [loading]);
  const { usd: ethUsd } = useEthPrice();
  const navigate = useNavigate();

  const [sort, setSort] = useState<{ key: 'hash' | 'block_number' | 'timestamp' | 'from_address' | 'to_address' | 'value' | 'status' | null; direction: 'asc' | 'desc'; }>({ key: null, direction: 'desc' });
  const seenTxRef = useRef<Set<string>>(new Set());
  const initializedRef = useRef(false);
  const [highlightTxs, setHighlightTxs] = useState<Set<string>>(new Set());
  const timeoutsRef = useRef<Map<string, number>>(new Map());

  useEffect(() => {
    if (!autoRefresh) return;
    const id = setInterval(() => {
      if (!loading) void refetch();
    }, 1000);
    return () => clearInterval(id);
  }, [autoRefresh, refetch, loading]);

  useEffect(() => {
    try { localStorage.setItem('txs:autoRefresh', String(autoRefresh)); } catch {}
  }, [autoRefresh]);

  // Age ticker
  useEffect(() => {
    const id = setInterval(() => setTick((t) => (t + 1) % 1_000_000), 1000);
    return () => clearInterval(id);
  }, []);

  // New tx highlights
  useEffect(() => {
    if (!transactions.length) return;
    if (!initializedRef.current) {
      for (const tx of transactions) seenTxRef.current.add(tx.hash);
      initializedRef.current = true;
      return;
    }
    if (!autoRefresh) return;
    const newOnes: string[] = [];
    for (const tx of transactions) if (!seenTxRef.current.has(tx.hash)) newOnes.push(tx.hash);
    if (newOnes.length) {
      setHighlightTxs((prev) => new Set([...prev, ...newOnes]));
      for (const h of newOnes) {
        seenTxRef.current.add(h);
        const t = window.setTimeout(() => {
          setHighlightTxs((prev) => { const n = new Set(prev); n.delete(h); return n; });
          timeoutsRef.current.delete(h);
        }, 1200);
        timeoutsRef.current.set(h, t);
      }
    }
  }, [transactions, autoRefresh]);

  useEffect(() => () => { for (const [,t] of timeoutsRef.current) clearTimeout(t); timeoutsRef.current.clear(); }, []);

  const handleSort = (key: 'hash' | 'block_number' | 'timestamp' | 'from_address' | 'to_address' | 'value' | 'status') => {
    setSort((prev) => prev.key === key ? { key, direction: prev.direction === 'asc' ? 'desc' : 'asc' } : { key, direction: key === 'block_number' || key === 'timestamp' || key === 'value' ? 'desc' : 'asc' });
  };

  const sortedTxs = useMemo(() => {
    if (!sort.key) return transactions;
    const dir = sort.direction === 'asc' ? 1 : -1;
    return [...transactions].sort((a, b) => {
      const k = sort.key!;
      if (k === 'hash' || k === 'from_address') return a[k].localeCompare(b[k]) * dir;
      if (k === 'to_address') {
        const av = a.to_address ?? '';
        const bv = b.to_address ?? '';
        return av.localeCompare(bv) * dir;
      }
      if (k === 'value') {
        const av = BigInt(a.value);
        const bv = BigInt(b.value);
        return av === bv ? 0 : av < bv ? -1 * dir : 1 * dir;
      }
      if (k === 'status') {
        const av = a.status ? 1 : 0;
        const bv = b.status ? 1 : 0;
        return av === bv ? 0 : av < bv ? -1 * dir : 1 * dir;
      }
      const av = a[k] as unknown as number;
      const bv = b[k] as unknown as number;
      return av === bv ? 0 : av < bv ? -1 * dir : 1 * dir;
    });
  }, [transactions, sort]);

  // Classify transaction type with simple heuristics
  const classify = (tx: import('../types').Transaction): string => {
    try {
      if (tx.contract_created) return 'Contract Creation';
      const hasData = tx.input_data && tx.input_data !== '0x';
      const hasValue = BigInt(tx.value) > 0n;
      if (!hasData) return 'ETH Transfer';
      const sig = tx.input_data.slice(0, 10).toLowerCase();
      if (sig === '0xa9059cbb') return 'ERC-20 Transfer';
      if (sig === '0x23b872dd') return 'ERC-20 TransferFrom';
      if (sig === '0x095ea7b3') return 'ERC-20 Approve';
      if (sig === '0x42842e0e' || sig === '0xb88d4fde') return 'ERC-721 Transfer';
      if (sig === '0x2eb2c2d6' || sig === '0xa22cb465') return 'SetApprovalForAll';
      if (sig === '0xf242432a') return 'ERC-1155 Transfer';
      if (hasValue) return 'Contract Call + ETH';
      return 'Contract Interaction';
    } catch {
      return 'Transaction';
    }
  };

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-white">Transactions</h1>
        <div className="flex items-center gap-3">
          <button
            onClick={() => setAutoRefresh((v) => !v)}
            className={`btn ${autoRefresh ? 'btn-primary' : 'btn-secondary'} flex items-center justify-center`}
            aria-pressed={autoRefresh}
            title={autoRefresh ? 'Pause live updates' : 'Start live updates'}
          >
            {autoRefresh ? (
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 9v6M14 9v6" />
              </svg>
            ) : (
              <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
                <path d="M8 5v14l11-7-11-7z" />
              </svg>
            )}
          </button>
        </div>
      </div>

      <div className="card overflow-hidden">
        {loading && !hasLoaded ? (
          <div className="py-10"><Loading size="sm" /></div>
        ) : (
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="bg-dark-700">
                <th className="table-cell text-left table-header">
                  <button className="flex items-center gap-1 hover:text-white" onClick={() => handleSort('hash')}>
                    Tx Hash
                    {sort.key === 'hash' && (
                      <svg className="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                        {sort.direction === 'asc' ? (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
                        ) : (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                        )}
                      </svg>
                    )}
                  </button>
                </th>
                <th className="table-cell text-left table-header">Type</th>
                <th className="table-cell text-left table-header">
                  <button className="flex items-center gap-1 hover:text-white" onClick={() => handleSort('block_number')}>
                    Block
                    {sort.key === 'block_number' && (
                      <svg className="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                        {sort.direction === 'asc' ? (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
                        ) : (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                        )}
                      </svg>
                    )}
                  </button>
                </th>
                <th className="table-cell text-left table-header">
                  <button className="flex items-center gap-1 hover:text-white" onClick={() => handleSort('timestamp')}>
                    Age
                    {sort.key === 'timestamp' && (
                      <svg className="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                        {sort.direction === 'asc' ? (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
                        ) : (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                        )}
                      </svg>
                    )}
                  </button>
                </th>
                <th className="table-cell text-left table-header">
                  <button className="flex items-center gap-1 hover:text-white" onClick={() => handleSort('from_address')}>
                    From
                    {sort.key === 'from_address' && (
                      <svg className="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                        {sort.direction === 'asc' ? (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
                        ) : (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                        )}
                      </svg>
                    )}
                  </button>
                </th>
                <th className="table-cell text-left table-header">
                  <button className="flex items-center gap-1 hover:text-white" onClick={() => handleSort('to_address')}>
                    To
                    {sort.key === 'to_address' && (
                      <svg className="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                        {sort.direction === 'asc' ? (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
                        ) : (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                        )}
                      </svg>
                    )}
                  </button>
                </th>
                <th className="table-cell text-right table-header">
                  <button className="flex items-center gap-1 ml-auto hover:text-white" onClick={() => handleSort('value')}>
                    Value
                    {sort.key === 'value' && (
                      <svg className="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                        {sort.direction === 'asc' ? (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
                        ) : (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                        )}
                      </svg>
                    )}
                  </button>
                </th>
                <th className="table-cell text-center table-header">
                  <button className="flex items-center gap-1 justify-center hover:text-white" onClick={() => handleSort('status')}>
                    Status
                    {sort.key === 'status' && (
                      <svg className="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                        {sort.direction === 'asc' ? (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
                        ) : (
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                        )}
                      </svg>
                    )}
                  </button>
                </th>
              </tr>
            </thead>
            <tbody>
              {sortedTxs.map((tx) => (
                <tr
                  key={tx.hash}
                  tabIndex={0}
                  role="button"
                  onClick={() => navigate(`/tx/${tx.hash}`)}
                  onKeyDown={(e) => { if (e.key === 'Enter') navigate(`/tx/${tx.hash}`); }}
                  className={`hover:bg-dark-600/70 transition-colors cursor-pointer ${highlightTxs.has(tx.hash) ? 'row-highlight' : ''}`}
                >
                  <td className="table-cell">
                    <span className="font-mono text-xs text-white">{truncateHash(tx.hash, 10, 8)}</span>
                  </td>
                  <td className="table-cell">
                    <span className="inline-flex items-center px-2 py-0.5 rounded-full border text-[10px] font-semibold bg-dark-600 text-gray-200 border-dark-500">
                      {classify(tx)}
                    </span>
                  </td>
                  <td className="table-cell">
                    <BlockLink blockNumber={tx.block_number} />
                  </td>
                  <td className="table-cell text-gray-400 text-xs">
                    {formatTimeAgo(tx.timestamp)}
                  </td>
                  <td className="table-cell">
                    <AddressLink address={tx.from_address} />
                  </td>
                  <td className="table-cell">
                    {tx.to_address ? (
                      <AddressLink address={tx.to_address} />
                    ) : (
                      <span className="text-gray-500 text-xs">Contract Creation</span>
                    )}
                  </td>
                  <td className="table-cell text-right font-mono text-xs text-gray-200">
                    {(() => {
                      const ethStr = formatEtherExact(tx.value);
                      const ethNum = Number(ethStr);
                      const usdStr = ethUsd != null ? formatUsd(ethNum * ethUsd) : null;
                      return usdStr ? `${ethStr} ETH (${usdStr})` : `${ethStr} ETH`;
                    })()}
                  </td>
                  <td className="table-cell text-center">
                    <StatusBadge status={tx.status} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        )}
      </div>

      {/* Compact pager like Blocks page */}
      <div className="mt-4">
        <div className="flex items-center justify-center gap-2">
          <button
            className="btn btn-secondary text-xs"
            onClick={() => setPage(1)}
            disabled={page === 1}
            aria-label="First page"
            title="First page"
          >
            <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5v14" />
            </svg>
          </button>
          <button
            className="btn btn-secondary text-xs"
            onClick={() => setPage(Math.max(1, page - 1))}
            disabled={page === 1}
            aria-label="Previous page"
            title="Previous page"
          >
            <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <span className="btn btn-secondary text-xs font-mono cursor-default pointer-events-none">
            {sortedTxs.length > 0
              ? `${formatNumber((pagination?.page ? (pagination.page - 1) : 0) * (pagination?.limit ?? 20) + 1)} – ${formatNumber((pagination?.page ? (pagination.page - 1) : 0) * (pagination?.limit ?? 20) + sortedTxs.length)}`
              : '—'}
          </span>
          <button
            className="btn btn-secondary text-xs"
            onClick={() => pagination && setPage(Math.min(pagination.total_pages, page + 1))}
            disabled={!pagination || page === pagination?.total_pages}
            aria-label="Next page"
            title="Next page"
          >
            <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
            </svg>
          </button>
          <button
            className="btn btn-secondary text-xs"
            onClick={() => pagination && setPage(pagination.total_pages)}
            disabled={!pagination || page === pagination?.total_pages}
            aria-label="Last page"
            title="Last page"
          >
            <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 5v14" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}
