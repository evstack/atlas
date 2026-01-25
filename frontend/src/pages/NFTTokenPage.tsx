import { useParams, Link } from 'react-router-dom';
import { useNftToken, useNftContract } from '../hooks';
import { AddressLink, CopyButton } from '../components';
import { truncateHash, formatNumber } from '../utils';

export default function NFTTokenPage() {
  const { contract: contractAddress, tokenId } = useParams<{ contract: string; tokenId: string }>();

  const { contract } = useNftContract(contractAddress);
  const { token } = useNftToken(contractAddress, tokenId);

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
            <div className="aspect-square bg-dark-700 flex items-center justify-center">
              {token?.image_url ? (
                <img
                  src={token.image_url}
                  alt={token.name || `Token #${token.token_id}`}
                  className="w-full h-full object-contain"
                  onError={(e) => {
                    (e.target as HTMLImageElement).style.display = 'none';
                  }}
                />
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

            <h1 className="text-2xl font-bold text-white mb-2">
              {token?.name || (tokenId ? `Token #${tokenId}` : 'NFT Token')}
            </h1>

            {token?.metadata?.description && (
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
    </div>
  );
}
