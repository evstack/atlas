import { useContext, useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useBlocks, useFeatures } from '../hooks';
import { CopyButton, EntityHeroVisual, Loading, PageHero } from '../components';
import { formatNumber, formatTimeAgo, formatGas, truncateHash } from '../utils';
import { BlockStatsContext } from '../context/BlockStatsContext';
import type { BlockDaStatus } from '../types';

const BLOCKS_PER_PAGE = 20;

function isDaIncluded(status: Pick<BlockDaStatus, 'header_da_height' | 'data_da_height'> | null | undefined): boolean {
  return !!status && status.header_da_height > 0 && status.data_da_height > 0;
}

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
  const { blocks: fetchedBlocks, pagination, refetch, loading } = useBlocks({ page, limit: BLOCKS_PER_PAGE });
  const features = useFeatures();
  const hasLoaded = !loading || pagination !== null;
  const { latestBlockEvent, sseConnected, subscribeDa, subscribeDaResync } = useContext(BlockStatsContext);
  const [daOverrides, setDaOverrides] = useState<Map<number, BlockDaStatus>>(new Map());
  const [daHighlight, setDaHighlight] = useState<Set<number>>(new Set());
  const daOverridesRef = useRef<Map<number, BlockDaStatus>>(new Map());
  const daOverridesSyncRafRef = useRef<number | null>(null);
  const daHighlightTimeoutsRef = useRef<Map<number, number>>(new Map());
  const baseDaIncludedRef = useRef<Map<number, boolean>>(new Map());
  const visibleDaBlocksRef = useRef<Set<number>>(new Set());
  const bufferedDaBlocksRef = useRef<Set<number>>(new Set());
  const [, setTick] = useState(0);
  const [sseBlocks, setSseBlocks] = useState<typeof fetchedBlocks>([]);
  const lastSseBlockRef = useRef<number | null>(null);
  const ssePrependRafRef = useRef<number | null>(null);
  const pendingSseBlocksRef = useRef<typeof fetchedBlocks>([]);
  const sseFilterRafRef = useRef<number | null>(null);
  const freshBlocksResetRafRef = useRef<number | null>(null);
  const [freshBlocks, setFreshBlocks] = useState<Set<number>>(new Set());
  const freshBlockTimeoutsRef = useRef<Map<number, number>>(new Map());

  const cancelDaOverridesSync = () => {
    if (daOverridesSyncRafRef.current !== null) {
      cancelAnimationFrame(daOverridesSyncRafRef.current);
      daOverridesSyncRafRef.current = null;
    }
  };

  // Cache fetched block numbers to avoid recreating Sets on every effect/memo
  const fetchedNumberSet = useMemo(
    () => new Set(fetchedBlocks.map((b) => b.number)),
    [fetchedBlocks],
  );

  // Prepend new blocks from SSE on page 1 with auto-refresh.
  // Buffer pending blocks so that burst arrivals (e.g. 100, 101, 102 before the
  // next frame) are all flushed in a single RAF rather than cancelling each other.
  useEffect(() => {
    if (!latestBlockEvent || page !== 1 || !autoRefresh) return;
    const block = latestBlockEvent.block;
    if (lastSseBlockRef.current != null && block.number <= lastSseBlockRef.current) return;
    lastSseBlockRef.current = block.number;
    pendingSseBlocksRef.current.push(block);
    bufferedDaBlocksRef.current.add(block.number);
    if (ssePrependRafRef.current !== null) return; // RAF already scheduled; block is buffered
    ssePrependRafRef.current = window.requestAnimationFrame(() => {
      const pending = pendingSseBlocksRef.current;
      pendingSseBlocksRef.current = [];
      const newlyPrepended: number[] = [];
      setSseBlocks((prev) => {
        const seen = new Set(prev.map((b) => b.number));
        const prepend: typeof prev = [];
        for (const b of pending) {
          if (seen.has(b.number)) continue;
          seen.add(b.number);
          prepend.push(b);
          newlyPrepended.push(b.number);
        }
        prepend.reverse();
        return [...prepend, ...prev].slice(0, BLOCKS_PER_PAGE);
      });
      if (newlyPrepended.length > 0) {
        setFreshBlocks((prev) => {
          const next = new Set(prev);
          for (const blockNumber of newlyPrepended) {
            next.add(blockNumber);
          }
          return next;
        });
        for (const blockNumber of newlyPrepended) {
          const existing = freshBlockTimeoutsRef.current.get(blockNumber);
          if (existing !== undefined) clearTimeout(existing);
          const timeoutId = window.setTimeout(() => {
            setFreshBlocks((prev) => {
              const next = new Set(prev);
              next.delete(blockNumber);
              return next;
            });
            freshBlockTimeoutsRef.current.delete(blockNumber);
          }, 1400);
          freshBlockTimeoutsRef.current.set(blockNumber, timeoutId);
        }
      }
      ssePrependRafRef.current = null;
    });
  }, [latestBlockEvent, page, autoRefresh]);

  useEffect(() => {
    if (freshBlocksResetRafRef.current !== null) {
      cancelAnimationFrame(freshBlocksResetRafRef.current);
      freshBlocksResetRafRef.current = null;
    }

    if (page !== 1 || !autoRefresh) {
      bufferedDaBlocksRef.current = new Set();
      for (const [, timeoutId] of freshBlockTimeoutsRef.current) clearTimeout(timeoutId);
      freshBlockTimeoutsRef.current.clear();
      freshBlocksResetRafRef.current = window.requestAnimationFrame(() => {
        setFreshBlocks((prev) => (prev.size === 0 ? prev : new Set()));
        freshBlocksResetRafRef.current = null;
      });
      return;
    }

    const next = new Set<number>(sseBlocks.map((block) => block.number));
    for (const block of pendingSseBlocksRef.current) {
      next.add(block.number);
    }
    bufferedDaBlocksRef.current = next;
  }, [autoRefresh, page, sseBlocks]);

  // Drop SSE blocks that are now present in fetchedBlocks to avoid duplicates,
  // but keep any that haven't been fetched yet.
  useEffect(() => {
    if (!fetchedBlocks.length) return;
    if (sseFilterRafRef.current !== null) cancelAnimationFrame(sseFilterRafRef.current);
    sseFilterRafRef.current = window.requestAnimationFrame(() => {
      setSseBlocks((prev) => prev.filter((b) => !fetchedNumberSet.has(b.number)));
      sseFilterRafRef.current = null;
    });
  }, [fetchedBlocks, fetchedNumberSet]);

  // Merge: SSE blocks prepended, deduped, trimmed to page size.
  // On page 1, keep the merged snapshot even when auto-refresh is paused so
  // the table doesn't snap back to the last fetched poll result.
  const blocks = useMemo(() => {
    if (page !== 1 || !sseBlocks.length) return fetchedBlocks;
    const unique = sseBlocks.filter((b) => !fetchedNumberSet.has(b.number));
    return [...unique, ...fetchedBlocks]
      .sort((a, b) => b.number - a.number)
      .slice(0, BLOCKS_PER_PAGE);
  }, [fetchedBlocks, fetchedNumberSet, sseBlocks, page]);

  useEffect(() => {
    if (!features.da_tracking) {
      baseDaIncludedRef.current = new Map();
      visibleDaBlocksRef.current = new Set();
      if (daOverridesRef.current.size > 0) {
        const empty = new Map<number, BlockDaStatus>();
        daOverridesRef.current = empty;
        cancelDaOverridesSync();
        daOverridesSyncRafRef.current = window.requestAnimationFrame(() => {
          setDaOverrides(empty);
          daOverridesSyncRafRef.current = null;
        });
      }
      return;
    }

    const visible = new Set<number>();
    const next = new Map<number, boolean>();
    for (const block of blocks) {
      visible.add(block.number);
      next.set(block.number, isDaIncluded(block.da_status));
    }
    baseDaIncludedRef.current = next;
    visibleDaBlocksRef.current = visible;
    const buffered = bufferedDaBlocksRef.current;

    let changed = false;
    const nextOverrides = new Map<number, BlockDaStatus>();
    for (const [blockNumber, status] of daOverridesRef.current) {
      if (!visible.has(blockNumber) && !buffered.has(blockNumber)) {
        changed = true;
        continue;
      }
      nextOverrides.set(blockNumber, status);
    }

    if (changed || nextOverrides.size !== daOverridesRef.current.size) {
      daOverridesRef.current = nextOverrides;
      cancelDaOverridesSync();
      daOverridesSyncRafRef.current = window.requestAnimationFrame(() => {
        setDaOverrides(nextOverrides);
        daOverridesSyncRafRef.current = null;
      });
    }
  }, [blocks, features.da_tracking]);

  // Subscribe to DA updates from SSE. setState is called inside the subscription
  // callback (not synchronously in the effect body), satisfying react-hooks/set-state-in-effect.
  useEffect(() => {
    if (!features.da_tracking) return;
    return subscribeDa((updates) => {
      const visible = visibleDaBlocksRef.current;
      const buffered = bufferedDaBlocksRef.current;
      if (visible.size === 0 && buffered.size === 0) return;

      const next = new Map<number, BlockDaStatus>();
      for (const [blockNumber, status] of daOverridesRef.current) {
        if (visible.has(blockNumber) || buffered.has(blockNumber)) {
          next.set(blockNumber, status);
        }
      }

      const transitionedToIncluded: number[] = [];
      let changed = next.size !== daOverridesRef.current.size;

      for (const update of updates) {
        if (!visible.has(update.block_number) && !buffered.has(update.block_number)) continue;

        const prevStatus = next.get(update.block_number);
        const wasIncluded = prevStatus
          ? isDaIncluded(prevStatus)
          : (baseDaIncludedRef.current.get(update.block_number) ?? false);
        const nextStatus = {
          block_number: update.block_number,
          header_da_height: update.header_da_height,
          data_da_height: update.data_da_height,
          updated_at: new Date().toISOString(),
        };

        if (
          prevStatus?.header_da_height === nextStatus.header_da_height
          && prevStatus?.data_da_height === nextStatus.data_da_height
        ) {
          continue;
        }

        if (!wasIncluded && isDaIncluded(nextStatus)) {
          transitionedToIncluded.push(update.block_number);
        }

        next.set(update.block_number, nextStatus);
        changed = true;
      }

      if (!changed) return;

      cancelDaOverridesSync();
      daOverridesRef.current = next;
      setDaOverrides(next);

      // Flash dots for 1.5s only when status transitions from pending -> included.
      for (const blockNumber of transitionedToIncluded) {
        setDaHighlight((prev) => new Set(prev).add(blockNumber));
        const existing = daHighlightTimeoutsRef.current.get(blockNumber);
        if (existing !== undefined) clearTimeout(existing);
        const t = window.setTimeout(() => {
          setDaHighlight((prev) => {
            const nextHighlight = new Set(prev);
            nextHighlight.delete(blockNumber);
            return nextHighlight;
          });
          daHighlightTimeoutsRef.current.delete(blockNumber);
        }, 1500);
        daHighlightTimeoutsRef.current.set(blockNumber, t);
      }
    });
  }, [features.da_tracking, subscribeDa]);

  useEffect(() => {
    if (!features.da_tracking) return;
    return subscribeDaResync(() => {
      cancelDaOverridesSync();
      const empty = new Map<number, BlockDaStatus>();
      daOverridesRef.current = empty;
      setDaOverrides(empty);
      void refetch();
    });
  }, [features.da_tracking, refetch, subscribeDaResync]);

  const navigate = useNavigate();
  const [sort, setSort] = useState<{ key: 'number' | 'hash' | 'timestamp' | 'transaction_count' | 'gas_used' | null; direction: 'asc' | 'desc'; }>({ key: null, direction: 'desc' });

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

  // When live updates are re-enabled, resync immediately so any blocks that
  // arrived during the pause are fetched before SSE prepends continue.
  const prevAutoRefreshRef = useRef(autoRefresh);
  useEffect(() => {
    if (!prevAutoRefreshRef.current && autoRefresh) {
      void refetch();
    }
    prevAutoRefreshRef.current = autoRefresh;
  }, [autoRefresh, refetch]);

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

  // Cleanup on unmount
  useEffect(() => {
    const activeDaTimeouts = daHighlightTimeoutsRef.current;
    const activeFreshTimeouts = freshBlockTimeoutsRef.current;
    return () => {
      if (daOverridesSyncRafRef.current !== null) {
        cancelAnimationFrame(daOverridesSyncRafRef.current);
        daOverridesSyncRafRef.current = null;
      }
      if (ssePrependRafRef.current !== null) {
        cancelAnimationFrame(ssePrependRafRef.current);
        ssePrependRafRef.current = null;
        pendingSseBlocksRef.current = [];
      }
      if (sseFilterRafRef.current !== null) {
        cancelAnimationFrame(sseFilterRafRef.current);
        sseFilterRafRef.current = null;
      }
      if (freshBlocksResetRafRef.current !== null) {
        cancelAnimationFrame(freshBlocksResetRafRef.current);
        freshBlocksResetRafRef.current = null;
      }
      for (const [, t] of activeDaTimeouts) clearTimeout(t);
      activeDaTimeouts.clear();
      for (const [, t] of activeFreshTimeouts) clearTimeout(t);
      activeFreshTimeouts.clear();
    };
  }, []);

  return (
    <div className="space-y-6 fade-in-up">
      <PageHero
        compact
        title="Blocks"
        actions={
          <button
            onClick={() => setAutoRefresh((v) => !v)}
            className={`btn ${autoRefresh ? 'btn-primary' : 'btn-secondary'} flex items-center justify-center`}
            aria-pressed={autoRefresh}
            title={autoRefresh ? 'Pause live updates' : 'Start live updates'}
          >
            {autoRefresh ? 'Live On' : 'Live Off'}
          </button>
        }
        visual={<EntityHeroVisual kind="blocks" />}
      />

      <div className="table-shell">
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
                {features.da_tracking && (
                  <th className="table-cell text-center table-header">DA</th>
                )}
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
                  className={`hover:bg-dark-600/70 transition-colors cursor-pointer ${freshBlocks.has(block.number) ? 'fresh-block' : ''}`}
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
                    <span className="text-gray-300 text-xs">{block.transaction_count}</span>
                  </td>
                  <td className="table-cell text-gray-300 text-xs">
                    {formatGas(block.gas_used.toString())}
                  </td>
                  {features.da_tracking && (() => {
                    const daStatus = daOverrides.get(block.number) ?? block.da_status;
                    const flash = daHighlight.has(block.number);
                    const included = isDaIncluded(daStatus);
                    const includedTitle = daStatus
                      ? `Header: ${daStatus.header_da_height}, Data: ${daStatus.data_da_height}`
                      : 'DA included';
                    return (
                      <td className="table-cell text-center">
                        {included ? (
                          <span className={`w-2 h-2 rounded-full bg-green-400 inline-block${flash ? ' animate-da-pulse' : ''}`} title={includedTitle} />
                        ) : (
                          <span className={`w-2 h-2 rounded-full bg-yellow-400 inline-block${flash ? ' animate-da-pulse' : ''}`} title="Pending DA inclusion" />
                        )}
                      </td>
                    );
                  })()}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        )}
      </div>

      {/* Compact pager: centered, without a jump-to-oldest control. */}
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
        </div>
      </div>
    </div>
  );
}
