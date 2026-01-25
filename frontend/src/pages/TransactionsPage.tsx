import { useState } from 'react';
import { useTransactions } from '../hooks';
import { Pagination, AddressLink, TxHashLink, BlockLink, StatusBadge } from '../components';
import { formatNumber, formatTimeAgo, formatEther } from '../utils';

export default function TransactionsPage() {
  const [page, setPage] = useState(1);
  const { transactions, pagination } = useTransactions({ page, limit: 20 });

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-white">Transactions</h1>
        {pagination && pagination.total > 0 && (
          <p className="text-gray-400 text-sm">
            Total: {formatNumber(pagination.total)} transactions
          </p>
        )}
      </div>

      <div className="card overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="bg-dark-700">
                <th className="table-cell text-left table-header">Tx Hash</th>
                <th className="table-cell text-left table-header">Block</th>
                <th className="table-cell text-left table-header">Age</th>
                <th className="table-cell text-left table-header">From</th>
                <th className="table-cell text-left table-header">To</th>
                <th className="table-cell text-right table-header">Value</th>
                <th className="table-cell text-center table-header">Status</th>
              </tr>
            </thead>
            <tbody>
              {transactions.map((tx) => (
                <tr key={tx.hash} className="hover:bg-dark-700/50 transition-colors">
                  <td className="table-cell">
                    <TxHashLink hash={tx.hash} />
                  </td>
                  <td className="table-cell">
                    <BlockLink blockNumber={tx.block_number} />
                  </td>
                  <td className="table-cell text-gray-400 text-sm">
                    {formatTimeAgo(tx.timestamp)}
                  </td>
                  <td className="table-cell">
                    <AddressLink address={tx.from_address} />
                  </td>
                  <td className="table-cell">
                    {tx.to_address ? (
                      <AddressLink address={tx.to_address} />
                    ) : (
                      <span className="text-gray-500 text-sm">Contract Creation</span>
                    )}
                  </td>
                  <td className="table-cell text-right font-mono text-sm text-gray-200">
                    {formatEther(tx.value)} ETH
                  </td>
                  <td className="table-cell text-center">
                    <StatusBadge status={tx.status} />
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
