import { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useNftContract, useNftTokens, useNftCollectionTransfers } from '../hooks';
import { Pagination, CopyButton, AddressLink, TxHashLink, Loading } from '../components';
import ImageIpfs from '../components/ImageIpfs';
import { formatNumber, truncateHash, formatTimeAgo } from '../utils';

export default function NFTContractPage() {
  const { contract: contractAddress } = useParams<{ contract: string }>();
  const [page, setPage] = useState(1);
  const [txPage, setTxPage] = useState(1);

  const { contract } = useNftContract(contractAddress);
  const { tokens, pagination } = useNftTokens(contractAddress, { page, limit: 20 });
  const { transfers, pagination: txPagination, loading: txLoading } = useNftCollectionTransfers(contractAddress, { page: txPage, limit: 20 });

  const [activeTab, setActiveTab] = useState<'tokens' | 'transfers'>('tokens');

  return (
    <div>
      {/* Header */}
      <div className="flex items-center space-x-4 mb-6">
        <div>
          <div className="flex items-center space-x-3">
            <h1 className="text-2xl font-bold text-fg">
              {contract?.name || 'NFT Collection'}
            </h1>
          </div>
          {contractAddress && (
            <div className="flex items-center space-x-2 mt-2">
              <span className="hash text-gray-400 text-sm">{contractAddress}</span>
              <CopyButton text={contractAddress} />
            </div>
          )}
        </div>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-2 md:grid-cols-3 gap-4 mb-8">
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Total Supply</p>
          <p className="text-xl font-semibold text-fg">
            {contract?.total_supply !== null && contract?.total_supply !== undefined
              ? formatNumber(contract.total_supply)
              : (pagination?.total !== undefined
                  ? formatNumber(pagination.total)
                  : '---')}
          </p>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Symbol</p>
          <p className="text-xl font-semibold text-fg">{contract?.symbol || '---'}</p>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">First Seen Block</p>
          <p className="text-xl font-semibold text-fg">
            {contract?.first_seen_block ? formatNumber(contract.first_seen_block) : '---'}
          </p>
        </div>
      </div>

      {/* Tabs */}
      <div className="border-b border-dark-600 mb-4">
        <nav className="flex space-x-8">
          <button
            onClick={() => setActiveTab('tokens')}
            className={`pb-4 px-1 border-b-2 font-medium text-sm transition-colors ${activeTab === 'tokens' ? 'border-accent-primary text-accent-primary' : 'border-transparent text-gray-400 hover:text-gray-300 hover:border-gray-300'}`}
          >
            Tokens {pagination?.total ? <span className="ml-1 text-gray-500">({formatNumber(pagination.total)})</span> : null}
          </button>
          <button
            onClick={() => setActiveTab('transfers')}
            className={`pb-4 px-1 border-b-2 font-medium text-sm transition-colors ${activeTab === 'transfers' ? 'border-accent-primary text-accent-primary' : 'border-transparent text-gray-400 hover:text-gray-300 hover:border-gray-300'}`}
          >
            Transfers {txPagination?.total ? <span className="ml-1 text-gray-500">({formatNumber(txPagination.total)})</span> : null}
          </button>
        </nav>
      </div>

      {activeTab === 'tokens' && (
        <>
          <div className="card">
            <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4 min-h-[200px]">
              {tokens.map((token) => {
                const imageUrl = token.image_url || token.token_uri || null;
                const displayName = token.name || `${contract?.name || contract?.symbol || 'NFT'} #${token.token_id}`;
                return (
                  <Link
                    key={token.token_id}
                    to={`/nfts/${contractAddress}/${token.token_id}`}
                    className="bg-dark-700 p-4 hover:bg-dark-600 transition-colors group"
                  >
                    <div className="aspect-square bg-dark-600 mb-3 flex items-center justify-center overflow-hidden">
                      {imageUrl ? (
                        <ImageIpfs srcUrl={imageUrl} alt={displayName} className="w-full h-full object-cover" />
                      ) : (
                        <span className="text-gray-500 text-4xl">#</span>
                      )}
                    </div>

                    <div>
                      <p className="text-fg font-medium truncate">{displayName}</p>
                      <p className="text-gray-400 text-sm mt-1">ID: {truncateHash(token.token_id, 8, 6)}</p>
                      <p className="text-gray-500 text-xs mt-1">Owner: {truncateHash(token.owner)}</p>
                    </div>
                  </Link>
                );
              })}
            </div>
          </div>
          {pagination && pagination.total_pages > 1 && (
            <Pagination currentPage={pagination.page} totalPages={pagination.total_pages} onPageChange={setPage} />
          )}
        </>
      )}

      {activeTab === 'transfers' && (
        <div className="card overflow-hidden">
          {txLoading ? (
            <div className="py-10"><Loading size="sm" /></div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="bg-dark-700">
                    <th className="table-cell text-left table-header">Tx Hash</th>
                    <th className="table-cell text-left table-header">Token</th>
                    <th className="table-cell text-left table-header">From</th>
                    <th className="table-cell text-left table-header">To</th>
                    <th className="table-cell text-left table-header">Block</th>
                    <th className="table-cell text-left table-header">Age</th>
                  </tr>
                </thead>
                <tbody>
                  {transfers.map((t) => (
                    <tr key={`${t.tx_hash}-${t.log_index}`} className="hover:bg-dark-700/50 transition-colors">
                      <td className="table-cell"><TxHashLink hash={t.tx_hash} /></td>
                      <td className="table-cell">
                        <Link to={`/nfts/${t.contract_address}/${t.token_id}`} className="text-accent-primary hover:underline">#{truncateHash(t.token_id, 8, 6)}</Link>
                      </td>
                      <td className="table-cell"><AddressLink address={t.from_address} /></td>
                      <td className="table-cell"><AddressLink address={t.to_address} /></td>
                      <td className="table-cell">{formatNumber(t.block_number)}</td>
                      <td className="table-cell text-gray-400 text-sm">{formatTimeAgo(t.timestamp)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {txPagination && txPagination.total_pages > 1 && (
            <Pagination currentPage={txPagination.page} totalPages={txPagination.total_pages} onPageChange={setTxPage} />
          )}
        </div>
      )}
    </div>
  );
}
