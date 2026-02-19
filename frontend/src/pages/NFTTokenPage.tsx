import { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useNftToken, useNftContract, useNftTokenTransfers } from '../hooks';
import { AddressLink, CopyButton, Pagination } from '../components';
import ImageIpfs from '../components/ImageIpfs';
import { truncateHash, formatNumber, formatTimeAgo } from '../utils';

export default function NFTTokenPage() {
  const { contract: contractAddress, tokenId } = useParams<{ contract: string; tokenId: string }>();

  const { contract } = useNftContract(contractAddress);
  const { token } = useNftToken(contractAddress, tokenId);
  const [txPage, setTxPage] = useState(1);
  const { transfers, pagination, loading } = useNftTokenTransfers(contractAddress, tokenId, { page: txPage, limit: 20 });

  const imageUrl = token?.image_url || token?.token_uri || null;
  const displayName = token?.name || `${contract?.name || contract?.symbol || 'NFT'} #${token?.token_id || tokenId || ''}`;

  return (
    <div>
      {/* Breadcrumb */}
      <div className="flex items-center space-x-2 text-sm text-gray-400 mb-6">
        <Link to="/nfts" className="hover:text-white">NFTs</Link>
        <span>/</span>
        <Link to={`/nfts/${contractAddress}`} className="hover:text-white">
          {contract?.name || (contractAddress ? truncateHash(contractAddress) : '---')}
        </Link>
        <span>/</span>
        <span className="text-white">#{tokenId ? truncateHash(tokenId, 8, 6) : '---'}</span>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
        {/* Image */}
        <div>
          <div className="card p-0 overflow-hidden">
            <div className="aspect-square bg-dark-700 flex items-center justify-center p-4">
              {imageUrl ? (
                <ImageIpfs
                  srcUrl={imageUrl}
                  alt={displayName}
                  className="w-full h-full object-contain"
                />
              ) : token && token.metadata_fetched === false ? (
                <div className="text-center text-sm">
                  <div className="inline-flex items-center px-3 py-2 rounded-md border border-dark-500 bg-dark-800 text-gray-300">
                    <svg className="w-4 h-4 mr-2 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01M12 5a7 7 0 100 14 7 7 0 000-14z" />
                    </svg>
                    Metadata has not been fetched yet.
                  </div>
                  <p className="text-gray-500 mt-2">Please check back shortly.</p>
                </div>
              ) : (
                <div className="text-center">
                  <span className="text-gray-500 text-8xl">#</span>
                  <p className="text-gray-500 mt-4">No image available</p>
                </div>
              )}
            </div>
          </div>

          {/* Attributes */}
          {token?.metadata?.attributes && token.metadata.attributes.length > 0 && (
            <div className="card mt-4">
              <h3 className="text-lg font-semibold text-white mb-4">Attributes</h3>
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
                {token.metadata.attributes.map((attr, index) => (
                  <div key={index} className="bg-dark-700 p-3 text-center">
                    <p className="text-gray-500 text-xs uppercase tracking-wider">
                      {attr.trait_type}
                    </p>
                    <p className="text-white font-medium mt-1 truncate">{attr.value}</p>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Details */}
        <div>
          <div className="card">
            <div className="flex items-center space-x-3 mb-4">
              <Link
                to={`/nfts/${contractAddress}`}
                className="text-accent-primary hover:underline"
              >
                {contract?.name || 'NFT Collection'}
              </Link>
            </div>

            <h1 className="text-2xl font-bold text-white mb-2">{displayName}</h1>

            {token && token.metadata_fetched === false && !imageUrl ? (
              <div className="mt-2">
                <div className="inline-flex items-center px-2.5 py-1 rounded-md border border-dark-500 bg-dark-700 text-gray-300 text-xs">
                  <svg className="w-4 h-4 mr-1 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01M12 5a7 7 0 100 14 7 7 0 000-14z" />
                  </svg>
                  Metadata has not been fetched yet.
                </div>
              </div>
            ) : token?.metadata?.description && (
              <p className="text-gray-400 mt-4">{token.metadata.description}</p>
            )}
          </div>

          <div className="card mt-4">
            <h3 className="text-lg font-semibold text-white mb-4">Details</h3>
            <dl className="space-y-4">
              <div className="flex flex-col sm:flex-row sm:items-start">
                <dt className="text-gray-400 sm:w-32 flex-shrink-0">Token ID:</dt>
                <dd className="flex items-center space-x-2">
                  <span className="hash text-gray-200">{token?.token_id || tokenId || '---'}</span>
                  {(token?.token_id || tokenId) && <CopyButton text={token?.token_id || tokenId || ''} />}
                </dd>
              </div>
              <div className="flex flex-col sm:flex-row sm:items-start">
                <dt className="text-gray-400 sm:w-32 flex-shrink-0">Contract:</dt>
                <dd className="flex items-center space-x-2">
                  {token?.contract_address || contractAddress ? (
                    <>
                      <AddressLink address={token?.contract_address || contractAddress || ''} truncate={false} />
                      <CopyButton text={token?.contract_address || contractAddress || ''} />
                    </>
                  ) : (
                    <span className="text-gray-200">---</span>
                  )}
                </dd>
              </div>
              <div className="flex flex-col sm:flex-row sm:items-start">
                <dt className="text-gray-400 sm:w-32 flex-shrink-0">Owner:</dt>
                <dd className="flex items-center space-x-2">
                  {token?.owner ? (
                    <>
                      <AddressLink address={token.owner} truncate={false} />
                      <CopyButton text={token.owner} />
                    </>
                  ) : (
                    <span className="text-gray-200">---</span>
                  )}
                </dd>
              </div>
              <div className="flex flex-col sm:flex-row sm:items-start">
                <dt className="text-gray-400 sm:w-32 flex-shrink-0">Last Transfer:</dt>
                <dd className="text-gray-200">
                  {token?.last_transfer_block ? `Block ${formatNumber(token.last_transfer_block)}` : '---'}
                </dd>
              </div>
              {token?.token_uri && (
                <div className="flex flex-col sm:flex-row sm:items-start">
                  <dt className="text-gray-400 sm:w-32 flex-shrink-0">Token URI:</dt>
                  <dd className="break-all">
                    <a
                      href={token.token_uri}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-accent-primary hover:underline text-sm"
                    >
                      {truncateHash(token.token_uri, 30, 10)}
                    </a>
                  </dd>
                </div>
              )}
            </dl>
          </div>
        </div>
      </div>

      {/* Transfers */}
      <div className="card mt-6 overflow-hidden">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-white">Transfer History</h2>
          {pagination && (
            <span className="text-gray-400 text-sm">{formatNumber(pagination.total)} transfers</span>
          )}
        </div>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="bg-dark-700">
                <th className="table-cell text-left table-header">Tx Hash</th>
                <th className="table-cell text-left table-header">From</th>
                <th className="table-cell text-left table-header">To</th>
                <th className="table-cell text-left table-header">Block</th>
                <th className="table-cell text-left table-header">Age</th>
              </tr>
            </thead>
            <tbody>
              {loading ? (
                <tr><td className="table-cell" colSpan={5}>Loading...</td></tr>
              ) : transfers.length === 0 ? (
                <tr><td className="table-cell text-sm text-gray-400" colSpan={5}>No transfers found.</td></tr>
              ) : (
                transfers.map(t => (
                  <tr key={`${t.tx_hash}-${t.log_index}`} className="hover:bg-dark-700/50 transition-colors">
                    <td className="table-cell"><Link to={`/tx/${t.tx_hash}`} className="address">{truncateHash(t.tx_hash, 10, 8)}</Link></td>
                    <td className="table-cell"><AddressLink address={t.from_address} /></td>
                    <td className="table-cell"><AddressLink address={t.to_address} /></td>
                    <td className="table-cell">{formatNumber(t.block_number)}</td>
                    <td className="table-cell text-gray-400 text-sm">{formatTimeAgo(t.timestamp)}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {pagination && pagination.total_pages > 1 && (
          <Pagination currentPage={pagination.page} totalPages={pagination.total_pages} onPageChange={setTxPage} />
        )}
      </div>
    </div>
  );
}
