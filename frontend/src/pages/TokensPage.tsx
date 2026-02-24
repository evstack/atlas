import { useState, useMemo } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useTokens } from '../hooks';
import { Pagination, Loading } from '../components';
import { formatNumber, truncateHash } from '../utils';

export default function TokensPage() {
  const [page, setPage] = useState(1);
  const { tokens, pagination, loading } = useTokens({ page, limit: 20 });
  const navigate = useNavigate();
  const hasLoaded = !loading || pagination !== null;

  const [sort, setSort] = useState<{ key: 'name' | 'first_seen_block' | 'decimals' | null; direction: 'asc' | 'desc'; }>({ key: 'first_seen_block', direction: 'desc' });
  const handleSort = (key: 'name' | 'first_seen_block' | 'decimals') => {
    setSort((prev) => (prev.key === key ? { key, direction: prev.direction === 'asc' ? 'desc' : 'asc' } : { key, direction: key === 'first_seen_block' ? 'desc' : 'asc' }));
  };

  const sortedTokens = useMemo(() => {
    if (!sort.key) return tokens;
    const dir = sort.direction === 'asc' ? 1 : -1;
    return [...tokens].sort((a, b) => {
      if (sort.key === 'name') {
        const an = (a.name || '').toLowerCase();
        const bn = (b.name || '').toLowerCase();
        return an.localeCompare(bn) * dir;
      }
      if (sort.key === 'first_seen_block') {
        const av = a.first_seen_block;
        const bv = b.first_seen_block;
        return av === bv ? 0 : av < bv ? -1 * dir : 1 * dir;
      }
      if (sort.key === 'decimals') {
        const av = a.decimals;
        const bv = b.decimals;
        return av === bv ? 0 : av < bv ? -1 * dir : 1 * dir;
      }
      return 0;
    });
  }, [tokens, sort]);

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-white">ERC-20 Tokens</h1>
        {pagination && pagination.total > 0 && (
          <p className="text-gray-400 text-sm">
            Total: {formatNumber(pagination.total)} tokens
          </p>
        )}
      </div>

      <div className="card overflow-hidden">
          {loading && !hasLoaded ? (
            <div className="py-10"><Loading size="sm" /></div>
          ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-xs">
              <thead>
                <tr className="bg-dark-700">
                  <th className="table-cell text-left table-header text-xs">
                    <button className="flex items-center gap-1 hover:text-white" onClick={() => handleSort('name')}>
                      Token
                      {sort.key === 'name' && (
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
                  <th className="table-cell text-left table-header text-xs">Contract</th>
                  <th className="table-cell text-right table-header text-xs">
                    <button className="flex items-center gap-1 ml-auto hover:text-white" onClick={() => handleSort('decimals')}>
                      Decimals
                      {sort.key === 'decimals' && (
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
                  <th className="table-cell text-right table-header text-xs">
                    <button className="flex items-center gap-1 ml-auto hover:text-white" onClick={() => handleSort('first_seen_block')}>
                      First Seen Block
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
                </tr>
              </thead>
              <tbody>
                {sortedTokens.map((token) => (
                  <tr
                    key={token.address}
                    tabIndex={0}
                    role="button"
                    onClick={() => navigate(`/tokens/${token.address}`)}
                    onKeyDown={(e) => { if (e.key === 'Enter') navigate(`/tokens/${token.address}`); }}
                    className="hover:bg-dark-700/50 transition-colors cursor-pointer"
                  >
                    <td className="table-cell py-1">
                      <Link to={`/tokens/${token.address}`} className="flex items-center gap-2 hover:underline">
                        <span className="text-white font-medium truncate">{token.name || 'Unknown Token'}</span>
                        <span className="inline-flex items-center px-1.5 py-0 rounded-full border border-dark-500 bg-dark-600 text-[10px] text-gray-300 uppercase tracking-wide">
                          {token.symbol || '—'}
                        </span>
                      </Link>
                    </td>
                    <td className="table-cell py-1">
                      <Link to={`/address/${token.address}`} className="address">
                        {truncateHash(token.address)}
                      </Link>
                    </td>
                    <td className="table-cell py-1 text-right text-gray-300">{token.decimals}</td>
                    <td className="table-cell py-1 text-right text-gray-300">{formatNumber(token.first_seen_block)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          )}
        </div>

      {pagination && pagination.total_pages > 1 && (
        <Pagination
          currentPage={pagination.page}
          totalPages={pagination.total_pages}
          onPageChange={setPage}
        />
      )}
    </div>
  );
}
