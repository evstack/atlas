import { useState, useEffect } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useAddress, useAddressTransactions, useEthBalance, useAddressTokens, useAddressNfts, useAddressTransfers } from '../hooks';
import { Pagination, TxHashLink, BlockLink, StatusBadge, AddressLink, CopyButton, ContractTypeBadge } from '../components';
import ImageIpfs from '../components/ImageIpfs';
import { formatNumber, formatTimeAgo, formatEtherExact, formatTokenAmount, formatUsd, formatTokenAmountExact } from '../utils';
import { useEthPrice } from '../hooks';

type TabType = 'transactions' | 'tokens' | 'nfts' | 'transfers';

export default function AddressPage() {
  const { address: addressParam } = useParams<{ address: string }>();
  const [activeTab, setActiveTab] = useState<TabType>('transactions');
  const [page, setPage] = useState(1);
  const [tokensPage, setTokensPage] = useState(1);
  const [nftsPage, setNftsPage] = useState(1);
  const [transfersPage, setTransfersPage] = useState(1);

  const { address, loading: addressLoading, error: addressError } = useAddress(addressParam);
  const { transactions, pagination } = useAddressTransactions(addressParam, {
    page,
    limit: 20,
  });
  const { balanceWei } = useEthBalance(addressParam);
  const { balances, pagination: tokensPagination } = useAddressTokens(addressParam, { page: tokensPage, limit: 20 });
  const { usd: ethUsd } = useEthPrice();
  const { tokens: nfts, pagination: nftsPagination } = useAddressNfts(addressParam, { page: nftsPage, limit: 24 });
  const { transfers, pagination: transfersPagination, loading: transfersLoading } = useAddressTransfers(addressParam, { page: transfersPage, limit: 20 });
  const [tokenMeta, setTokenMeta] = useState<Record<string, { decimals: number }>>({});

  const tabs: { id: TabType; label: string; count?: number }[] = [
    { id: 'transactions', label: 'Transactions', count: pagination?.total },
    { id: 'transfers', label: 'Transfers', count: transfersPagination?.total },
    { id: 'tokens', label: 'Tokens', count: tokensPagination?.total },
    { id: 'nfts', label: 'NFTs', count: nftsPagination?.total },
  ];

  // Fetch ERC-20 decimals for transfers on this page
  // Keeps UI exact for token amounts; defaults to 18 if unavailable
  // Use dynamic import for tokens API to avoid circular imports at module scope
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const unique = Array.from(new Set(
        transfers
          .filter(t => t.transfer_type === 'erc20')
          .map(t => t.contract_address.toLowerCase())
      ));
      if (unique.length === 0) return;
      const { getToken } = await import('../api/tokens');
      const updates: Record<string, { decimals: number }> = {};
      for (const addr of unique) {
        try {
          const t = await getToken(addr);
          updates[addr] = { decimals: t.decimals };
        } catch {
          updates[addr] = { decimals: 18 };
        }
      }
      if (!cancelled) setTokenMeta(prev => ({ ...prev, ...updates }));
    })();
    return () => { cancelled = true; };
  }, [transfers]);


  // Not found state: show a friendly message rather than empty page
  if (!addressLoading && !address) {
    return (
      <div>
        <div className="flex flex-col gap-2 mb-6">
          <h1 className="text-2xl font-bold text-white">Address</h1>
          {addressParam && (
            <div className="flex items-center space-x-2 bg-dark-700 px-3 py-1 w-fit">
              <span className="hash text-gray-300 text-sm">{addressParam}</span>
              <CopyButton text={addressParam} />
            </div>
          )}
        </div>
        <div className="card p-4">
          <p className="text-gray-200 font-medium">This address does not exist.</p>
          {addressError?.error && (
            <p className="text-gray-500 text-sm mt-1">{addressError.error}</p>
          )}
          <p className="text-gray-400 text-sm mt-2">Check the address and try again.</p>
        </div>
      </div>
    );
  }

  return (
    <div>
      {/* Header with Label */}
      <div className="flex flex-col gap-2 mb-6">
        <div className="flex items-center space-x-3">
          <h1 className="text-2xl font-bold text-white">
            {address?.address_type === 'erc20'
              ? 'Token Contract'
              : address?.address_type === 'nft'
              ? 'NFT Contract'
              : address?.address_type === 'contract'
              ? 'Contract'
              : 'Address'}
          </h1>
          <ContractTypeBadge type={address?.address_type} />
          {address?.address_type === 'nft' && addressParam && (
            <Link to={`/nfts/${addressParam}`} className="btn btn-secondary text-xs" title="Open NFT Collection">
              View Collection
            </Link>
          )}
          {address?.address_type === 'erc20' && addressParam && (
            <Link to={`/tokens/${addressParam}`} className="btn btn-secondary text-xs" title="Open Token Details">
              View Token
            </Link>
          )}
        </div>
        {addressParam && (
          <div className="flex items-center space-x-2 bg-dark-700 px-3 py-1 w-fit">
            <span className="hash text-gray-300 text-sm">{addressParam}</span>
            <CopyButton text={addressParam} />
          </div>
        )}
      </div>

      {/* Removed proxy badge as requested */}

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
          <p className="text-gray-400 text-sm mb-1">ETH Balance</p>
          <p className="text-xl font-semibold text-white">{balanceWei ? `${formatEtherExact(balanceWei)} ETH` : '---'}</p>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Address Type</p>
          <div className="text-white font-semibold">
            {address?.address_type === 'erc20'
              ? 'ERC-20 Token'
              : address?.address_type === 'nft'
              ? 'NFT Collection'
              : address?.address_type === 'contract'
              ? 'Contract'
              : 'EOA'}
          </div>
          {(address?.address_type === 'erc20' || address?.address_type === 'nft') && (address?.name || address?.symbol) && (
            <div className="text-gray-300 text-sm mt-1">
              {address?.name || ''}{address?.symbol ? ` (${address.symbol})` : ''}
            </div>
          )}
          {address?.address_type === 'erc20' && address.total_supply && typeof address.decimals === 'number' && (
            <div className="text-gray-400 text-sm mt-2">
              Total Supply: <span className="text-gray-200 font-mono">{formatTokenAmountExact(address.total_supply, address.decimals)}</span>
            </div>
          )}
          {address?.address_type === 'nft' && address.total_supply && (
            <div className="text-gray-400 text-sm mt-2">
              Total Supply: <span className="text-gray-200 font-mono">{formatNumber(address.total_supply)}</span>
            </div>
          )}
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
                  <th className="table-cell text-left table-header">Direction</th>
                  <th className="table-cell text-right table-header">Value</th>
                  <th className="table-cell text-center table-header">Status</th>
                </tr>
              </thead>
              <tbody>
                {transactions.map((tx) => {
                  const self = addressParam?.toLowerCase();
                  const isSender = tx.from_address.toLowerCase() === self;
                  const badge = (
                    <span className="inline-flex items-center px-2 py-0.5 rounded-full border text-xs font-medium bg-dark-600 text-white border-dark-500">
                      {isSender ? 'Sent to' : 'Received from'}
                    </span>
                  );
                  const counterparty = isSender
                    ? (tx.to_address ? <AddressLink address={tx.to_address} /> : <span className="text-gray-500 text-sm">Contract Creation</span>)
                    : <AddressLink address={tx.from_address} />;
                  return (
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
                        <div className="flex items-center gap-2">
                          {badge}
                          {counterparty}
                        </div>
                      </td>
                      <td className="table-cell text-right font-mono text-sm text-gray-200">
                        {(() => {
                          const ethStr = formatEtherExact(tx.value);
                          const ethNum = Number(ethStr);
                          const usdStr = ethUsd != null ? formatUsd(ethNum * ethUsd) : null;
                          return usdStr ? `${ethStr} ETH (${usdStr})` : `${ethStr} ETH`;
                        })()}
                      </td>
                      <td className="table-cell text-center">
                        <StatusBadge status={tx.status} />
                      </td>
                    </tr>
                  );
                })}
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

      {/* Tokens Tab */}
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
                      <Link to={`/tokens/${balance.contract_address}`} className="hover:underline">
                        <div className="flex flex-col">
                          <span className="text-white font-medium">{balance.name || 'Unknown Token'}</span>
                          <span className="text-gray-500 text-sm">{balance.symbol || '---'}</span>
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

      {/* NFTs Tab */}
      {activeTab === 'nfts' && (
        <div className="card">
          {nfts.length === 0 ? (
            <p className="text-gray-400 text-sm">No NFTs found for this account.</p>
          ) : (
            <>
              <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-4">
                {nfts.map((t) => {
                  const imageUrl = t.image_url || t.token_uri || null;
                  const displayName = t.name || `#${t.token_id}`;
                  return (
                    <Link key={`${t.contract_address}-${t.token_id}`} to={`/nfts/${t.contract_address}/${t.token_id}`} className="block group">
                      <div className="aspect-square bg-dark-700 border border-dark-600 rounded-lg overflow-hidden flex items-center justify-center">
                        {imageUrl ? (
                          <ImageIpfs srcUrl={imageUrl} alt={displayName} className="w-full h-full object-cover group-hover:opacity-90 transition-opacity" />
                        ) : (
                          <span className="text-gray-500 text-xs">No Image</span>
                        )}
                      </div>
                      <div className="mt-2">
                        <div className="text-white text-sm truncate">{displayName}</div>
                        <div className="text-gray-500 text-xs truncate">{t.contract_address}</div>
                      </div>
                    </Link>
                  );
                })}
              </div>

              {nftsPagination && nftsPagination.total_pages > 1 && (
                <div className="mt-4 flex items-center justify-center gap-2">
                  <button className="btn btn-secondary text-xs" onClick={() => setNftsPage(1)} disabled={nftsPagination.page === 1} aria-label="First page" title="First page">
                    <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" /><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5v14" /></svg>
                  </button>
                  <button className="btn btn-secondary text-xs" onClick={() => setNftsPage(Math.max(1, nftsPagination.page - 1))} disabled={nftsPagination.page === 1} aria-label="Previous page" title="Previous page">
                    <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" /></svg>
                  </button>
                  <span className="btn btn-secondary text-xs font-mono cursor-default pointer-events-none">
                    {(nftsPagination.page - 1) * nftsPagination.limit + 1} – {(nftsPagination.page - 1) * nftsPagination.limit + nfts.length}
                  </span>
                  <button className="btn btn-secondary text-xs" onClick={() => setNftsPage(Math.min(nftsPagination.total_pages, nftsPagination.page + 1))} disabled={nftsPagination.page === nftsPagination.total_pages} aria-label="Next page" title="Next page">
                    <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" /></svg>
                  </button>
                  <button className="btn btn-secondary text-xs" onClick={() => setNftsPage(nftsPagination.total_pages)} disabled={nftsPagination.page === nftsPagination.total_pages} aria-label="Last page" title="Last page">
                    <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" /><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 5v14" /></svg>
                  </button>
                </div>
              )}
            </>
          )}
        </div>
      )}

      {/* Transfers Tab */}
      {activeTab === 'transfers' && (
        <div className="card overflow-hidden">
          <div className="overflow-x-auto">
            {transfersLoading && (!transfers || transfers.length === 0) ? (
              <div className="py-8"><span className="text-gray-400 text-sm">Loading transfers…</span></div>
            ) : transfers.length === 0 ? (
              <p className="text-gray-400 text-sm">No token transfers found for this address.</p>
            ) : (
              <table className="w-full">
                <thead>
                  <tr className="bg-dark-700">
                    <th className="table-cell text-left table-header">Type</th>
                    <th className="table-cell text-left table-header">Token</th>
                    <th className="table-cell text-left table-header">Contract</th>
                    <th className="table-cell text-left table-header">From</th>
                    <th className="table-cell text-left table-header">To</th>
                    <th className="table-cell text-right table-header">Amount / Token ID</th>
                    <th className="table-cell text-left table-header">Tx</th>
                    <th className="table-cell text-left table-header">Block</th>
                  </tr>
                </thead>
                <tbody>
                  {transfers.map((t, idx) => {
                    const isErc20 = t.transfer_type === 'erc20';
                    const decimals = isErc20 ? (tokenMeta[t.contract_address.toLowerCase()]?.decimals ?? 18) : undefined;
                    const amount = isErc20 ? `${formatTokenAmountExact(t.value, decimals!)} ${t.token_symbol || ''}` : `#${t.value}`;
                    return (
                      <tr key={`${t.tx_hash}-${t.log_index}-${idx}`} className="hover:bg-dark-700/50 transition-colors">
                        <td className="table-cell text-xs text-gray-200 uppercase">{isErc20 ? 'ERC-20' : 'NFT'}</td>
                        <td className="table-cell">
                          <div className="flex flex-col">
                            <span className="text-white text-sm">{t.token_name || (isErc20 ? 'ERC-20' : 'NFT')}</span>
                            <span className="text-gray-500 text-xs">{t.token_symbol || ''}</span>
                          </div>
                        </td>
                        <td className="table-cell"><AddressLink address={t.contract_address} /></td>
                        <td className="table-cell"><AddressLink address={t.from_address} /></td>
                        <td className="table-cell"><AddressLink address={t.to_address} /></td>
                        <td className="table-cell text-right font-mono text-xs text-gray-200">{amount}</td>
                        <td className="table-cell"><TxHashLink hash={t.tx_hash} /></td>
                        <td className="table-cell"><BlockLink blockNumber={t.block_number} /></td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </div>
          {transfersPagination && transfersPagination.total_pages > 1 && (
            <Pagination currentPage={transfersPagination.page} totalPages={transfersPagination.total_pages} onPageChange={setTransfersPage} />
          )}
        </div>
      )}

    </div>
  );
}
