import { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useAddress, useAddressTransactions, useAddressTokens, useContractProxy, useLabel } from '../hooks';
import { Pagination, TxHashLink, BlockLink, StatusBadge, AddressLink, CopyButton, ProxyBadge } from '../components';
import { formatNumber, formatTimeAgo, formatEther, formatTokenAmount } from '../utils';

type TabType = 'transactions' | 'tokens';

export default function AddressPage() {
  const { address: addressParam } = useParams<{ address: string }>();
  const [activeTab, setActiveTab] = useState<TabType>('transactions');
  const [page, setPage] = useState(1);
  const [tokensPage, setTokensPage] = useState(1);

  const { address } = useAddress(addressParam);
  const { transactions, pagination } = useAddressTransactions(addressParam, {
    page,
    limit: 20,
  });
  const { balances, pagination: tokensPagination } = useAddressTokens(addressParam, {
    page: tokensPage,
    limit: 20,
  });
  const { proxyInfo } = useContractProxy(address?.is_contract ? addressParam : undefined);
  const { label } = useLabel(addressParam);

  const tabs: { id: TabType; label: string; count?: number }[] = [
    { id: 'transactions', label: 'Transactions', count: pagination?.total },
    { id: 'tokens', label: 'Token Holdings', count: tokensPagination?.total },
  ];

  return (
    <div>
      {/* Header with Label */}
      <div className="flex flex-col gap-2 mb-6">
        <div className="flex items-center space-x-3">
          <h1 className="text-2xl font-bold text-white">
            {label ? label.name : address?.is_contract ? 'Contract' : 'Address'}
          </h1>
          {label && label.tags.length > 0 && (
            <div className="flex gap-1">
              {label.tags.slice(0, 3).map((tag) => (
                <span
                  key={tag}
                  className="text-accent-primary text-xs px-2 py-0.5 border border-accent-primary"
                >
                  {tag}
                </span>
              ))}
            </div>
          )}
        </div>
        {addressParam && (
          <div className="flex items-center space-x-2 bg-dark-700 px-3 py-1 w-fit">
            <span className="hash text-gray-300 text-sm">{addressParam}</span>
            <CopyButton text={addressParam} />
          </div>
        )}
        {label?.description && (
          <p className="text-gray-400 text-sm">{label.description}</p>
        )}
      </div>

      {/* Proxy Badge for Contracts */}
      {address?.is_contract && proxyInfo && addressParam && (
        <div className="card mb-4">
          <ProxyBadge address={addressParam} showImplementation={true} />
        </div>
      )}

      {/* Overview */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Transactions</p>
          <p className="text-xl font-semibold text-white">{address ? formatNumber(address.tx_count) : '---'}</p>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">First Seen Block</p>
          <p className="text-xl font-semibold text-white">{address ? formatNumber(address.first_seen_block) : '---'}</p>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Token Holdings</p>
          <p className="text-xl font-semibold text-white">
            {tokensPagination ? formatNumber(tokensPagination.total) : '---'}
          </p>
        </div>
        {address?.is_contract && (
          <div className="card">
            <p className="text-gray-400 text-sm mb-1">Type</p>
            <p className="text-xl font-semibold text-accent-primary">
              {proxyInfo ? 'Proxy Contract' : 'Smart Contract'}
            </p>
          </div>
        )}
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
              {tab.count !== undefined && (
                <span className="ml-2 text-gray-500">({formatNumber(tab.count)})</span>
              )}
            </button>
          ))}
        </nav>
      </div>

      {/* Transactions Tab */}
      {activeTab === 'transactions' && (
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
                      {tx.from_address.toLowerCase() === addressParam?.toLowerCase() ? (
                        <span className="text-gray-500 text-sm font-mono">OUT</span>
                      ) : (
                        <AddressLink address={tx.from_address} />
                      )}
                    </td>
                    <td className="table-cell">
                      {tx.to_address ? (
                        tx.to_address.toLowerCase() === addressParam?.toLowerCase() ? (
                          <span className="text-white text-sm font-mono">IN</span>
                        ) : (
                          <AddressLink address={tx.to_address} />
                        )
                      ) : (
                        <span className="text-gray-500 text-sm">Contract</span>
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
          {pagination && pagination.total_pages > 1 && (
            <Pagination
              currentPage={pagination.page}
              totalPages={pagination.total_pages}
              onPageChange={setPage}
            />
          )}
        </div>
      )}

      {/* Token Holdings Tab */}
      {activeTab === 'tokens' && (
        <div className="card overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="bg-dark-700">
                  <th className="table-cell text-left table-header">Token</th>
                  <th className="table-cell text-left table-header">Contract</th>
                  <th className="table-cell text-right table-header">Balance</th>
                </tr>
              </thead>
              <tbody>
                {balances.map((balance) => (
                  <tr key={balance.contract_address} className="hover:bg-dark-700/50 transition-colors">
                    <td className="table-cell">
                      <Link
                        to={`/tokens/${balance.contract_address}`}
                        className="hover:underline"
                      >
                        <div className="flex flex-col">
                          <span className="text-white font-medium">
                            {balance.name || 'Unknown Token'}
                          </span>
                          <span className="text-gray-500 text-sm">
                            {balance.symbol || '---'}
                          </span>
                        </div>
                      </Link>
                    </td>
                    <td className="table-cell">
                      <AddressLink address={balance.contract_address} />
                    </td>
                    <td className="table-cell text-right font-mono text-gray-200">
                      {formatTokenAmount(balance.balance, balance.decimals)} {balance.symbol || ''}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {tokensPagination && tokensPagination.total_pages > 1 && (
            <Pagination
              currentPage={tokensPagination.page}
              totalPages={tokensPagination.total_pages}
              onPageChange={setTokensPage}
            />
          )}
        </div>
      )}
    </div>
  );
}
