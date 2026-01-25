import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useTokens } from '../hooks';
import { Pagination } from '../components';
import { formatNumber, truncateHash, formatTokenAmount } from '../utils';

export default function TokensPage() {
  const [page, setPage] = useState(1);
  const { tokens, pagination } = useTokens({ page, limit: 20 });

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
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="bg-dark-700">
                  <th className="table-cell text-left table-header">#</th>
                  <th className="table-cell text-left table-header">Token</th>
                  <th className="table-cell text-left table-header">Contract</th>
                  <th className="table-cell text-right table-header">Decimals</th>
                  <th className="table-cell text-right table-header">Total Supply</th>
                </tr>
              </thead>
              <tbody>
                {tokens.map((token, index) => (
                  <tr key={token.address} className="hover:bg-dark-700/50 transition-colors">
                    <td className="table-cell text-gray-400">
                      {(pagination ? (pagination.page - 1) * pagination.limit : 0) + index + 1}
                    </td>
                    <td className="table-cell">
                      <Link
                        to={`/tokens/${token.address}`}
                        className="hover:underline"
                      >
                        <div className="flex flex-col">
                          <span className="text-white font-medium">
                            {token.name || 'Unknown Token'}
                          </span>
                          <span className="text-gray-500 text-sm">
                            {token.symbol || '---'}
                          </span>
                        </div>
                      </Link>
                    </td>
                    <td className="table-cell">
                      <Link
                        to={`/address/${token.address}`}
                        className="address"
                      >
                        {truncateHash(token.address)}
                      </Link>
                    </td>
                    <td className="table-cell text-right text-gray-300">
                      {token.decimals}
                    </td>
                    <td className="table-cell text-right text-gray-300 font-mono">
                      {token.total_supply
                        ? formatTokenAmount(token.total_supply, token.decimals)
                        : '---'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
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
