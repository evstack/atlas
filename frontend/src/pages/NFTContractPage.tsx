import { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useNftContract, useNftTokens } from '../hooks';
import { Pagination, CopyButton } from '../components';
import { formatNumber, truncateHash } from '../utils';

export default function NFTContractPage() {
  const { contract: contractAddress } = useParams<{ contract: string }>();
  const [page, setPage] = useState(1);

  const { contract } = useNftContract(contractAddress);
  const { tokens, pagination } = useNftTokens(contractAddress, { page, limit: 20 });

  return (
    <div>
      {/* Header */}
      <div className="flex items-center space-x-4 mb-6">
        <div>
          <div className="flex items-center space-x-3">
            <h1 className="text-2xl font-bold text-white">
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
          <p className="text-xl font-semibold text-white">
            {contract?.total_supply !== null && contract?.total_supply !== undefined
              ? formatNumber(contract.total_supply)
              : '---'}
          </p>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Symbol</p>
          <p className="text-xl font-semibold text-white">{contract?.symbol || '---'}</p>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">First Seen Block</p>
          <p className="text-xl font-semibold text-white">
            {contract?.first_seen_block ? formatNumber(contract.first_seen_block) : '---'}
          </p>
        </div>
      </div>

      {/* Tokens */}
      <div className="card">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-white">Tokens</h2>
          {pagination && pagination.total > 0 && (
            <p className="text-gray-400 text-sm">
              {formatNumber(pagination.total)} tokens
            </p>
          )}
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4 min-h-[200px]">
          {tokens.map((token) => (
            <Link
              key={token.token_id}
              to={`/nfts/${contractAddress}/${token.token_id}`}
              className="bg-dark-700 p-4 hover:bg-dark-600 transition-colors group"
            >
              {/* Token image placeholder */}
              <div className="aspect-square bg-dark-600 mb-3 flex items-center justify-center overflow-hidden">
                {token.image_url ? (
                  <img
                    src={token.image_url}
                    alt={token.name || `Token #${token.token_id}`}
                    className="w-full h-full object-cover"
                    onError={(e) => {
                      (e.target as HTMLImageElement).style.display = 'none';
                    }}
                  />
                ) : (
                  <span className="text-gray-500 text-4xl">#</span>
                )}
              </div>

              <div>
                <p className="text-white font-medium truncate">
                  {token.name || `Token #${token.token_id}`}
                </p>
                <p className="text-gray-400 text-sm mt-1">
                  ID: {truncateHash(token.token_id, 8, 6)}
                </p>
                <p className="text-gray-500 text-xs mt-1">
                  Owner: {truncateHash(token.owner)}
                </p>
              </div>
            </Link>
          ))}
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
