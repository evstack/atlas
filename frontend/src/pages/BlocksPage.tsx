import { useContext, useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useBlocks } from '../hooks';
import { CopyButton, Loading } from '../components';
import { formatNumber, formatTimeAgo, formatGas, truncateHash } from '../utils';
import { BlockStatsContext } from '../context/BlockStatsContext';

export default function BlocksPage() {
  const [page, setPage] = useState(1);
  const [autoRefresh, setAutoRefresh] = useState<boolean>(() => {
    try {
      const v = localStorage.getItem('blocks:autoRefresh');
      return v === null ? true : v === 'true';
    } catch {
      return true;
    }
  });
  const { blocks: fetchedBlocks, pagination, refetch, loading } = useBlocks({ page, limit: 20 });
  const hasLoaded = !loading || pagination !== null;
  const { latestBlockEvent, sseConnected } = useContext(BlockStatsContext);
  const [sseBlocks, setSseBlocks] = useState<typeof fetchedBlocks>([]);
  const lastSseBlockRef = useRef<number | null>(null);
  const ssePrependRafRef = useRef<number | null>(null);
  const sseFilterRafRef = useRef<number | null>(null);

  // Prepend new blocks from SSE on page 1 with auto-refresh
  useEffect(() => {
    if (!latestBlockEvent || page !== 1 || !autoRefresh) return;
    const block = latestBlockEvent.block;
    if (lastSseBlockRef.current != null && block.number <= lastSseBlockRef.current) return;
    lastSseBlockRef.current = block.number;
    if (ssePrependRafRef.current !== null) cancelAnimationFrame(ssePrependRafRef.current);
    ssePrependRafRef.current = window.requestAnimationFrame(() => {
      setSseBlocks((prev) => {
        if (prev.some((b) => b.number === block.number)) return prev;
        return [block, ...prev].slice(0, 20);
      });
      ssePrependRafRef.current = null;
    });
  }, [latestBlockEvent, page, autoRefresh]);

  // Drop SSE blocks that are now present in fetchedBlocks to avoid duplicates,
  // but keep any that haven't been fetched yet.
  useEffect(() => {
    if (!fetchedBlocks.length) return;
    const fetched = new Set(fetchedBlocks.map((b) => b.number));
    if (sseFilterRafRef.current !== null) cancelAnimationFrame(sseFilterRafRef.current);
    sseFilterRafRef.current = window.requestAnimationFrame(() => {
      setSseBlocks((prev) => prev.filter((b) => !fetched.has(b.number)));
      sseFilterRafRef.current = null;
    });
  }, [fetchedBlocks]);

  // Merge: SSE blocks prepended, deduped, trimmed to page size
  const blocks = useMemo(() => {
    if (!sseBlocks.length) return fetchedBlocks;
    const seen = new Set(fetchedBlocks.map((b) => b.number));
    const unique = sseBlocks.filter((b) => !seen.has(b.number));
    return [...unique, ...fetchedBlocks].slice(0, 20);
  }, [fetchedBlocks, sseBlocks]);
  const navigate = useNavigate();
  const [sort, setSort] = useState<{ key: 'number' | 'hash' | 'timestamp' | 'transaction_count' | 'gas_used' | null; direction: 'asc' | 'desc'; }>({ key: null, direction: 'desc' });
  const seenBlocksRef = useRef<Set<number>>(new Set());
  const initializedRef = useRef(false);
  const [highlightBlocks, setHighlightBlocks] = useState<Set<number>>(new Set());
  const timeoutsRef = useRef<Map<number, number>>(new Map());
  const highlightRafRef = useRef<number | null>(null);
  const [, setTick] = useState(0);

  const handleSort = (key: 'number' | 'hash' | 'timestamp' | 'transaction_count' | 'gas_used') => {
    setSort((prev) => {
      if (prev.key === key) {
        return { key, direction: prev.direction === 'asc' ? 'desc' : 'asc' };
      }
      return { key, direction: key === 'number' || key === 'timestamp' || key === 'transaction_count' || key === 'gas_used' ? 'desc' : 'asc' };
    });
  };

  const sortedBlocks = useMemo(() => {
    if (!sort.key) return blocks;
    const dir = sort.direction === 'asc' ? 1 : -1;
    return [...blocks].sort((a, b) => {
      const key = sort.key!;
      if (key === 'hash') {
        return a.hash.localeCompare(b.hash) * dir;
      }
      const av = a[key] as unknown as number;
      const bv = b[key] as unknown as number;
      if (av === bv) return 0;
      return av < bv ? -1 * dir : 1 * dir;
    });
  }, [blocks, sort]);

  // No polling while SSE is connected — periodic refetches disrupt the smooth live flow.
  // Fall back to 1s polling only when SSE is disconnected.
  useEffect(() => {
    if (!autoRefresh || sseConnected) return;
    const id = setInterval(() => {
      if (!loading) void refetch();
    }, 1000);
    return () => clearInterval(id);
  }, [autoRefresh, refetch, loading, sseConnected]);

  // When SSE drops, immediately refetch to catch any blocks missed during the gap.
  const prevSseConnectedRef = useRef(sseConnected);
  useEffect(() => {
    if (prevSseConnectedRef.current && !sseConnected && autoRefresh) {
      void refetch();
    }
    prevSseConnectedRef.current = sseConnected;
  }, [sseConnected, refetch, autoRefresh]);

  // Keep relative timestamps (Age) updating even when auto refresh is paused
  useEffect(() => {
    const id = setInterval(() => setTick((t) => (t + 1) % 1_000_000), 1000);
    return () => clearInterval(id);
  }, []);

  // Persist autoRefresh preference
  useEffect(() => {
    try {
      localStorage.setItem('blocks:autoRefresh', String(autoRefresh));
    } catch {
      // Ignore storage write failures (e.g. private mode/quota).
    }
  }, [autoRefresh]);

  // Detect newly seen blocks and flash highlight
  useEffect(() => {
    if (!blocks.length) return;

    // On first load, mark current blocks as seen but do not highlight
    if (!initializedRef.current) {
      for (const b of blocks) {
        seenBlocksRef.current.add(b.number);
      }
      initializedRef.current = true;
      return;
    }

    // Subsequent updates: only highlight blocks not previously seen
    const newlyAdded: number[] = [];
    for (const b of blocks) {
      if (!seenBlocksRef.current.has(b.number)) {
        newlyAdded.push(b.number);
      }
    }
    if (newlyAdded.length) {
      if (highlightRafRef.current !== null) {
        window.cancelAnimationFrame(highlightRafRef.current);
      }
      for (const n of newlyAdded) {
        seenBlocksRef.current.add(n);
      }
      highlightRafRef.current = window.requestAnimationFrame(() => {
        setHighlightBlocks((prev) => new Set([...prev, ...newlyAdded]));
        for (const n of newlyAdded) {
          const t = window.setTimeout(() => {
            setHighlightBlocks((prev) => {
              const next = new Set(prev);
              next.delete(n);
              return next;
            });
            timeoutsRef.current.delete(n);
          }, 1600);
          timeoutsRef.current.set(n, t);
        }
        highlightRafRef.current = null;
      });
    }
  }, [blocks]);

  // Cleanup on unmount
  useEffect(() => {
    const activeTimeouts = timeoutsRef.current;
    return () => {
      if (highlightRafRef.current !== null) {
        window.cancelAnimationFrame(highlightRafRef.current);
        highlightRafRef.current = null;
      }
      if (ssePrependRafRef.current !== null) {
        cancelAnimationFrame(ssePrependRafRef.current);
        ssePrependRafRef.current = null;
      }
      if (sseFilterRafRef.current !== null) {
        cancelAnimationFrame(sseFilterRafRef.current);
        sseFilterRafRef.current = null;
      }
      for (const [, t] of activeTimeouts) clearTimeout(t);
      activeTimeouts.clear();
    };
  }, []);

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-fg">Blocks</h1>
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
                  <button className="flex items-center gap-1 hover:text-fg" onClick={() => handleSort('number')}>
                    Block
                    {sort.key === 'number' && (
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
                  <button className="flex items-center gap-1 hover:text-fg" onClick={() => handleSort('hash')}>
                    Hash
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
                <th className="table-cell text-left table-header">
                  <button className="flex items-center gap-1 hover:text-fg" onClick={() => handleSort('timestamp')}>
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
                  <button className="flex items-center gap-1 hover:text-fg" onClick={() => handleSort('transaction_count')}>
                    Txns
                    {sort.key === 'transaction_count' && (
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
                  <button className="flex items-center gap-1 hover:text-fg" onClick={() => handleSort('gas_used')}>
                    Gas Used
                    {sort.key === 'gas_used' && (
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
              {sortedBlocks.map((block) => (
                <tr
                  key={block.number}
                  tabIndex={0}
                  role="button"
                  onClick={() => navigate(`/blocks/${block.number}`)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') navigate(`/blocks/${block.number}`);
                  }}
                  className={`hover:bg-dark-600/70 transition-colors cursor-pointer ${highlightBlocks.has(block.number) ? 'row-highlight' : ''}`}
                >
                  <td className="table-cell">
                    <Link
                      to={`/blocks/${block.number}`}
                      className="text-accent-primary hover:underline font-mono text-xs"
                    >
                      {formatNumber(block.number)}
                    </Link>
                  </td>
                  <td className="table-cell">
                    <div className="flex items-center gap-1">
                      <span className="hash text-xs text-gray-300">{truncateHash(block.hash, 10, 8)}</span>
                      <CopyButton text={block.hash} />
                    </div>
                  </td>
                  <td className="table-cell text-gray-400 text-xs">
                    {formatTimeAgo(block.timestamp)}
                  </td>
                  <td className="table-cell">
                    <span className="text-gray-200 text-xs">{block.transaction_count}</span>
                  </td>
                  <td className="table-cell text-gray-400 font-mono text-xs">
                    {formatGas(block.gas_used.toString())}
                  </td>
                  
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        )}
      </div>

      {/* Compact pager: centered, with First/Prev and Next/Last around the visible range */}
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
            {sortedBlocks.length > 0
              ? `${formatNumber(sortedBlocks[0].number)} – ${formatNumber(sortedBlocks[sortedBlocks.length - 1].number)}`
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
