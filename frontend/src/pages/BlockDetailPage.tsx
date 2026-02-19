import { useParams, Link } from 'react-router-dom';
import { useBlock, useBlockTransactions } from '../hooks';
import { CopyButton, Loading, AddressLink, TxHashLink, StatusBadge } from '../components';
import { formatNumber, formatTimestamp, formatGas, truncateHash, formatTimeAgo, formatEther } from '../utils';
import { useState } from 'react';

export default function BlockDetailPage() {
  const { number } = useParams<{ number: string }>();
  const blockNumber = number ? parseInt(number, 10) : undefined;
  const { block, loading: blockLoading, error: blockError } = useBlock(blockNumber);
  const [txPage, setTxPage] = useState(1);
  const { transactions, pagination, loading } = useBlockTransactions(blockNumber, { page: txPage, limit: 20 });

  type DetailRow = { label: string; value: JSX.Element | string; stacked?: boolean };
  const details: DetailRow[] = block ? [
    { label: 'Block Height', value: formatNumber(block.number) },
    { label: 'Timestamp', value: formatTimestamp(block.timestamp) },
    { label: 'Transactions', value: block.transaction_count.toString() },
    {
      label: 'Block Hash',
      stacked: true,
      value: (
        <div className="flex items-center space-x-2">
          <span className="hash text-gray-200">{block.hash}</span>
          <CopyButton text={block.hash} />
        </div>
      ),
    },
    {
      label: 'Parent Hash',
      stacked: true,
      value: (
        <div className="flex items-center space-x-2">
          <Link
            to={`/blocks/${block.number - 1}`}
            className="hash text-accent-primary hover:underline"
          >
            {truncateHash(block.parent_hash, 20, 20)}
          </Link>
          <CopyButton text={block.parent_hash} />
        </div>
      ),
    },
    { label: 'Gas Used', value: formatGas(block.gas_used.toString()) },
    { label: 'Gas Limit', value: formatGas(block.gas_limit.toString()) },
  ] : [
    { label: 'Block Height', value: '---' },
    { label: 'Timestamp', value: '---' },
    { label: 'Transactions', value: '---' },
    { label: 'Block Hash', value: '---', stacked: true },
    { label: 'Parent Hash', value: '---', stacked: true },
    { label: 'Gas Used', value: '---' },
    { label: 'Gas Limit', value: '---' },
  ];

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-white">Block {blockNumber !== undefined ? `#${formatNumber(blockNumber)}` : ''}</h1>
        <div className="flex space-x-2">
          {blockNumber !== undefined && blockNumber > 0 && (
            <Link to={`/blocks/${blockNumber - 1}`} className="btn btn-secondary text-sm" aria-label="Previous block" title="Previous block">
              <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
              </svg>
            </Link>
          )}
          {blockNumber !== undefined && (
            <Link to={`/blocks/${blockNumber + 1}`} className="btn btn-secondary text-sm" aria-label="Next block" title="Next block">
              <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
              </svg>
            </Link>
          )}
        </div>
      </div>

      {!blockLoading && !block && (
        <div className="card p-4 mb-6">
          <p className="text-gray-200 font-medium">This block does not exist.</p>
          {blockError?.error && <p className="text-gray-500 text-sm mt-1">{blockError.error}</p>}
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        <aside className="lg:col-span-3">
          <div className="card p-3">
            <h2 className="text-base font-semibold text-white mb-3">Overview</h2>
            <dl className="space-y-2">
              {details.map(({ label, value, stacked }) => (
                <div key={label} className={`flex flex-col ${stacked ? '' : 'sm:flex-row sm:items-start'} leading-tight`}>
                  <dt className={`text-gray-400 flex-shrink-0 text-xs ${stacked ? 'mb-0.5' : 'sm:w-32 mb-0.5 sm:mb-0'}`}>{label}:</dt>
                  <dd className="text-gray-200 break-all text-xs">{value}</dd>
                </div>
              ))}
            </dl>
          </div>
        </aside>

        <section className="lg:col-span-9">
          <div className="card overflow-hidden">
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="bg-dark-700">
                    <th className="table-cell text-left table-header">Tx Hash</th>
                    <th className="table-cell text-left table-header">Age</th>
                    <th className="table-cell text-left table-header">From</th>
                    <th className="table-cell text-left table-header">To</th>
                    <th className="table-cell text-right table-header">Value</th>
                    <th className="table-cell text-center table-header">Status</th>
                  </tr>
                </thead>
                <tbody>
                  {loading ? (
                    <tr>
                      <td className="px-4 py-8" colSpan={6}>
                        <Loading size="sm" />
                      </td>
                    </tr>
                  ) : transactions.length === 0 ? (
                    <tr>
                      <td className="px-4 py-8 text-center text-gray-400 text-sm" colSpan={6}>
                        This block doesn’t contain any transactions.
                      </td>
                    </tr>
                  ) : (
                    transactions.map((tx) => (
                      <tr key={tx.hash} className="hover:bg-dark-700/50 transition-colors">
                        <td className="table-cell">
                          <TxHashLink hash={tx.hash} />
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
                          {formatEther(tx.value)} ETH
                        </td>
                        <td className="table-cell text-center">
                          <StatusBadge status={tx.status} />
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>

          {pagination && pagination.total_pages > 1 && (
            <div className="mt-4 flex items-center justify-center gap-2">
              <button
                className="btn btn-secondary text-xs"
                onClick={() => setTxPage(1)}
                disabled={pagination.page === 1}
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
                onClick={() => setTxPage(Math.max(1, pagination.page - 1))}
                disabled={pagination.page === 1}
                aria-label="Previous page"
                title="Previous page"
              >
                <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
                </svg>
              </button>
              <span className="btn btn-secondary text-xs font-mono cursor-default pointer-events-none">
                {formatNumber((pagination.page - 1) * pagination.limit + 1)} – {formatNumber((pagination.page - 1) * pagination.limit + transactions.length)}
              </span>
              <button
                className="btn btn-secondary text-xs"
                onClick={() => setTxPage(Math.min(pagination.total_pages, pagination.page + 1))}
                disabled={pagination.page === pagination.total_pages}
                aria-label="Next page"
                title="Next page"
              >
                <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                </svg>
              </button>
              <button
                className="btn btn-secondary text-xs"
                onClick={() => setTxPage(pagination.total_pages)}
                disabled={pagination.page === pagination.total_pages}
                aria-label="Last page"
                title="Last page"
              >
                <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 5v14" />
                </svg>
              </button>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
