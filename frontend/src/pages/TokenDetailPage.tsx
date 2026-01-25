import { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useToken, useTokenHolders, useTokenTransfers } from '../hooks';
import { Pagination, AddressLink, TxHashLink, CopyButton } from '../components';
import { formatNumber, formatTokenAmount, formatPercentage, formatTimeAgo, truncateHash } from '../utils';

type TabType = 'holders' | 'transfers';

export default function TokenDetailPage() {
  const { address } = useParams<{ address: string }>();
  const [activeTab, setActiveTab] = useState<TabType>('holders');
  const [holdersPage, setHoldersPage] = useState(1);
  const [transfersPage, setTransfersPage] = useState(1);

  const { token } = useToken(address);
  const { holders, pagination: holdersPagination } = useTokenHolders(address, { page: holdersPage, limit: 20 });
  const { transfers, pagination: transfersPagination } = useTokenTransfers(address, { page: transfersPage, limit: 20 });

  const tabs: { id: TabType; label: string; count?: number }[] = [
    { id: 'holders', label: 'Holders', count: holdersPagination?.total },
    { id: 'transfers', label: 'Transfers', count: transfersPagination?.total },
  ];

  return (
    <div>
      {/* Header */}
      <div className="flex items-center space-x-3 mb-6">
        <h1 className="text-2xl font-bold text-white">
          {token?.name || 'Token'}
        </h1>
        {token?.symbol && (
          <span className="bg-dark-700 px-3 py-1 text-gray-300 text-sm">
            {token.symbol}
          </span>
        )}
      </div>

      {/* Overview Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Contract Address</p>
          <div className="flex items-center space-x-2">
            {address ? (
              <>
                <Link to={`/address/${address}`} className="address text-sm">
                  {truncateHash(address, 8, 6)}
                </Link>
                <CopyButton text={address} />
              </>
            ) : (
              <span className="text-gray-200">---</span>
            )}
          </div>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Decimals</p>
          <p className="text-xl font-semibold text-white">{token?.decimals ?? '---'}</p>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Total Supply</p>
          <p className="text-xl font-semibold text-white font-mono">
            {token?.total_supply
              ? formatTokenAmount(token.total_supply, token.decimals)
              : '---'}
          </p>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Holders</p>
          <p className="text-xl font-semibold text-white">
            {holdersPagination ? formatNumber(holdersPagination.total) : '---'}
          </p>
        </div>
      </div>

      {/* Tabs */}
      <div className="border-b border-dark-600 mb-4">
        <nav className="flex space-x-8">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`pb-4 px-1 border-b-2 font-medium text-sm transition-colors ${
                activeTab === tab.id
                  ? 'border-accent-primary text-accent-primary'
                  : 'border-transparent text-gray-400 hover:text-gray-300 hover:border-gray-300'
              }`}
            >
              {tab.label}
              {tab.count !== undefined && tab.count > 0 && (
                <span className="ml-2 text-gray-500">({formatNumber(tab.count)})</span>
              )}
            </button>
          ))}
        </nav>
      </div>

      {/* Tab Content */}
      {activeTab === 'holders' && (
        <div className="card overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="bg-dark-700">
                  <th className="table-cell text-left table-header">Rank</th>
                  <th className="table-cell text-left table-header">Address</th>
                  <th className="table-cell text-right table-header">Balance</th>
                  <th className="table-cell text-right table-header">Percentage</th>
                </tr>
              </thead>
              <tbody>
                {holders.map((holder, index) => (
                  <tr key={holder.address} className="hover:bg-dark-700/50 transition-colors">
                    <td className="table-cell text-gray-400">
                      {(holdersPagination ? (holdersPagination.page - 1) * holdersPagination.limit : 0) + index + 1}
                    </td>
                    <td className="table-cell">
                      <AddressLink address={holder.address} />
                    </td>
                    <td className="table-cell text-right font-mono text-gray-200">
                      {formatTokenAmount(holder.balance, token?.decimals ?? 18)} {token?.symbol || ''}
                    </td>
                    <td className="table-cell text-right text-gray-300">
                      {formatPercentage(holder.percentage)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {holdersPagination && holdersPagination.total_pages > 1 && (
            <Pagination
              currentPage={holdersPagination.page}
              totalPages={holdersPagination.total_pages}
              onPageChange={setHoldersPage}
            />
          )}
        </div>
      )}

      {activeTab === 'transfers' && (
        <div className="card overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="bg-dark-700">
                  <th className="table-cell text-left table-header">Tx Hash</th>
                  <th className="table-cell text-left table-header">Age</th>
                  <th className="table-cell text-left table-header">From</th>
                  <th className="table-cell text-left table-header">To</th>
                  <th className="table-cell text-right table-header">Amount</th>
                </tr>
              </thead>
              <tbody>
                {transfers.map((transfer) => (
                  <tr key={`${transfer.tx_hash}-${transfer.log_index}`} className="hover:bg-dark-700/50 transition-colors">
                    <td className="table-cell">
                      <TxHashLink hash={transfer.tx_hash} />
                    </td>
                    <td className="table-cell text-gray-400 text-sm">
                      {formatTimeAgo(transfer.timestamp)}
                    </td>
                    <td className="table-cell">
                      <AddressLink address={transfer.from_address} />
                    </td>
                    <td className="table-cell">
                      <AddressLink address={transfer.to_address} />
                    </td>
                    <td className="table-cell text-right font-mono text-gray-200">
                      {formatTokenAmount(transfer.value, token?.decimals ?? 18)} {token?.symbol || ''}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {transfersPagination && transfersPagination.total_pages > 1 && (
            <Pagination
              currentPage={transfersPagination.page}
              totalPages={transfersPagination.total_pages}
              onPageChange={setTransfersPage}
            />
          )}
        </div>
      )}
    </div>
  );
}
