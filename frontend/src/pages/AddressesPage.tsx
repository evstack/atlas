import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAddresses } from '../hooks';
import { CopyButton, Loading, ContractTypeBadge } from '../components';
import { formatNumber, truncateHash } from '../utils';

export default function AddressesPage() {
  const [page, setPage] = useState(1);
  const [autoRefresh, setAutoRefresh] = useState<boolean>(() => {
    try {
      const v = localStorage.getItem('addresses:autoRefresh');
      return v === null ? true : v === 'true';
    } catch { return true; }
  });
  const { addresses, pagination, refetch, loading } = useAddresses({ page, limit: 20 });
  const [hasLoaded, setHasLoaded] = useState(false);
  useEffect(() => {
    if (!loading) setHasLoaded(true);
  }, [loading]);
  const navigate = useNavigate();
  const [sort, setSort] = useState<{ key: 'address' | 'address_type' | 'first_seen_block' | 'tx_count' | null; direction: 'asc' | 'desc'; }>({ key: 'tx_count', direction: 'desc' });
  const seenRef = useRef<Set<string>>(new Set());
  const initializedRef = useRef(false);
  const [highlight, setHighlight] = useState<Set<string>>(new Set());
  const timersRef = useRef<Map<string, number>>(new Map());

  useEffect(() => {
    if (!autoRefresh) return;
    const id = setInterval(() => { if (!loading) void refetch(); }, 1000);
    return () => clearInterval(id);
  }, [autoRefresh, refetch, loading]);

  useEffect(() => { try { localStorage.setItem('addresses:autoRefresh', String(autoRefresh)); } catch {} }, [autoRefresh]);

  useEffect(() => {
    if (!addresses.length) return;
    if (!initializedRef.current) {
      for (const a of addresses) seenRef.current.add(a.address);
      initializedRef.current = true;
      return;
    }
    const newOnes: string[] = [];
    for (const a of addresses) if (!seenRef.current.has(a.address)) newOnes.push(a.address);
    if (newOnes.length) {
      setHighlight((prev) => new Set([...prev, ...newOnes]));
      for (const h of newOnes) {
        seenRef.current.add(h);
        const t = window.setTimeout(() => {
          setHighlight((prev) => { const n = new Set(prev); n.delete(h); return n; });
          timersRef.current.delete(h);
        }, 900);
        timersRef.current.set(h, t);
      }
    }
  }, [addresses]);

  useEffect(() => () => { for (const [,t] of timersRef.current) clearTimeout(t); timersRef.current.clear(); }, []);

  const handleSort = (key: 'address' | 'address_type' | 'first_seen_block' | 'tx_count') => {
    setSort((prev) => prev.key === key ? { key, direction: prev.direction === 'asc' ? 'desc' : 'asc' } : { key, direction: key === 'tx_count' || key === 'first_seen_block' ? 'desc' : 'asc' });
  };

  const sorted = useMemo(() => {
    const dir = sort.direction === 'asc' ? 1 : -1;
    return [...addresses].sort((a, b) => {
      const k = sort.key;
      if (!k) return 0;
      if (k === 'address') return a.address.localeCompare(b.address) * dir;
      if (k === 'address_type') {
        const av = a.address_type || 'eoa';
        const bv = b.address_type || 'eoa';
        return av.localeCompare(bv) * dir;
      }
      const av = a[k] as unknown as number; const bv = b[k] as unknown as number;
      return av === bv ? 0 : av < bv ? -1 * dir : 1 * dir;
    });
  }, [addresses, sort]);

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-white">Addresses</h1>
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
                  <button className="flex items-center gap-1 hover:text-white" onClick={() => handleSort('address')}>
                    Address
                    {sort.key === 'address' && (
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
                  <button className="flex items-center gap-1 hover:text-white" onClick={() => handleSort('address_type')}>
                    Type
                    {sort.key === 'address_type' && (
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
                  <button className="flex items-center gap-1 hover:text-white" onClick={() => handleSort('first_seen_block')}>
                    First Seen
                    {sort.key === 'first_seen_block' && (
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
                  <button className="flex items-center gap-1 hover:text-white" onClick={() => handleSort('tx_count')}>
                    Txns
                    {sort.key === 'tx_count' && (
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
              {sorted.map((addr) => (
                <tr
                  key={addr.address}
                  tabIndex={0}
                  role="button"
                  onClick={() => navigate(`/address/${addr.address}`)}
                  onKeyDown={(e) => { if (e.key === 'Enter') navigate(`/address/${addr.address}`); }}
                  className={`hover:bg-dark-600/70 transition-colors cursor-pointer ${highlight.has(addr.address) ? 'row-highlight' : ''}`}
                >
                  <td className="table-cell">
                    <div className="flex items-center gap-1">
                      <span className="font-mono text-xs text-white">{truncateHash(addr.address, 10, 8)}</span>
                      <CopyButton text={addr.address} />
                    </div>
                  </td>
                  <td className="table-cell text-xs text-gray-300">
                    <ContractTypeBadge type={(addr.address_type || 'eoa') as any} />
                  </td>
                  <td className="table-cell text-xs text-gray-300">
                    {formatNumber(addr.first_seen_block)}
                  </td>
                  <td className="table-cell">
                    <span className="text-gray-200 text-xs">{formatNumber(addr.tx_count)}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        )}
      </div>

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
            {addresses.length > 0
              ? `${formatNumber((pagination?.page ? (pagination.page - 1) : 0) * (pagination?.limit ?? 20) + 1)} – ${formatNumber((pagination?.page ? (pagination.page - 1) : 0) * (pagination?.limit ?? 20) + addresses.length)}`
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
