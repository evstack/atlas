import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useNftContracts } from '../hooks';
import { EntityHeroVisual, Pagination, Loading, PageHero } from '../components';
import { formatNumber, truncateHash } from '../utils';

export default function NFTsPage() {
  const [page, setPage] = useState(1);
  const { contracts, pagination, loading } = useNftContracts({ page, limit: 20 });
  const hasLoaded = !loading || pagination !== null;

  return (
    <div className="space-y-6 fade-in-up">
      <PageHero
        compact
        title="NFT Collections"
        visual={<EntityHeroVisual kind="nfts" />}
      />

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 min-h-[200px]">
          {loading && !hasLoaded ? (
            <div className="col-span-full py-10 flex justify-center"><Loading size="sm" /></div>
          ) : contracts.map((contract) => (
            <Link
              key={contract.address}
              to={`/nfts/${contract.address}`}
              className="card transition-colors group"
            >
              <div className="flex items-start justify-between mb-3">
                <div>
                  <h3 className="text-fg font-semibold">
                    {contract.name || 'Unknown Collection'}
                  </h3>
                  <p className="text-gray-500 text-sm">{contract.symbol || '---'}</p>
                </div>
              </div>

              <div className="space-y-2 text-sm">
                <div className="flex justify-between">
                  <span className="text-gray-400">Contract:</span>
                  <span className="hash text-gray-300">{truncateHash(contract.address)}</span>
                </div>
                {contract.total_supply !== null && (
                  <div className="flex justify-between">
                    <span className="text-gray-400">Total Supply:</span>
                    <span className="text-gray-200">{formatNumber(contract.total_supply)}</span>
                  </div>
                )}
                <div className="flex justify-between">
                  <span className="text-gray-400">First Seen Block:</span>
                  <span className="text-gray-200">{formatNumber(contract.first_seen_block)}</span>
                </div>
              </div>
            </Link>
          ))}
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
