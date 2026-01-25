import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useBlocks } from '../hooks';
import { Pagination } from '../components';
import { formatNumber, formatTimeAgo, formatGas } from '../utils';

export default function BlocksPage() {
  const [page, setPage] = useState(1);
  const { blocks, pagination } = useBlocks({ page, limit: 20 });

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-white">Blocks</h1>
        {pagination && pagination.total > 0 && (
          <p className="text-gray-400 text-sm">
            Total: {formatNumber(pagination.total)} blocks
          </p>
        )}
      </div>

      <div className="card overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="bg-dark-700">
                <th className="table-cell text-left table-header">Block</th>
                <th className="table-cell text-left table-header">Age</th>
                <th className="table-cell text-left table-header">Txns</th>
                <th className="table-cell text-left table-header">Gas Used</th>
                <th className="table-cell text-left table-header">Gas Limit</th>
              </tr>
            </thead>
            <tbody>
              {blocks.map((block) => (
                <tr key={block.number} className="hover:bg-dark-700/50 transition-colors">
                  <td className="table-cell">
                    <Link
                      to={`/blocks/${block.number}`}
                      className="text-accent-primary hover:underline font-mono"
                    >
                      {formatNumber(block.number)}
                    </Link>
                  </td>
                  <td className="table-cell text-gray-400 text-sm">
                    {formatTimeAgo(block.timestamp)}
                  </td>
                  <td className="table-cell">
                    <span className="text-gray-200">{block.transaction_count}</span>
                  </td>
                  <td className="table-cell text-gray-400 font-mono text-sm">
                    {formatGas(block.gas_used.toString())}
                  </td>
                  <td className="table-cell text-gray-400 font-mono text-sm">
                    {formatGas(block.gas_limit.toString())}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {pagination && (
        <Pagination
          currentPage={pagination.page}
          totalPages={pagination.total_pages}
          onPageChange={setPage}
        />
      )}
    </div>
  );
}
