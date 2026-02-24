import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { useParams } from 'react-router-dom';
import { useTransaction, useTransactionDecodedLogs } from '../hooks';
import { AddressLink, BlockLink, StatusBadge, CopyButton, EventLogs, Loading } from '../components';
import { formatTimestamp, formatEtherExact, formatGas, formatGasPrice, formatNumber, truncateHash, formatTokenAmountExact } from '../utils';
import { getTxErc20Transfers, getTxNftTransfers } from '../api/transactions';
import { getToken } from '../api/tokens';

export default function TransactionDetailPage() {
  const { hash } = useParams<{ hash: string }>();
  const [logsPage, setLogsPage] = useState(1);
  const { transaction, loading: txLoading, error: txError } = useTransaction(hash);
  const { logs, pagination: logsPagination, loading: logsLoading } = useTransactionDecodedLogs(hash, { page: logsPage, limit: 50 });
  const [subTab, setSubTab] = useState<'tokens' | 'nfts' | 'logs'>('tokens');
  const [erc20, setErc20] = useState<Array<{ contract_address: string; from_address: string; to_address: string; value: string }>>([]);
  const [nfts, setNfts] = useState<Array<{ contract_address: string; token_id: string; from_address: string; to_address: string }>>([]);
  const [erc20Loading, setErc20Loading] = useState<boolean>(true);
  const [nftsLoading, setNftsLoading] = useState<boolean>(true);
  const [tokenMeta, setTokenMeta] = useState<Record<string, { symbol: string | null; decimals: number }>>({});
  const [showInput, setShowInput] = useState(false);

  const tokensCount = erc20.length;
  const nftsCount = nfts.length;
  const logsCount = logsPagination?.total ?? (logs ? logs.length : 0);

  useEffect(() => {
    let cancelled = false;
    if (!hash) return;
    (async () => {
      setErc20Loading(true);
      setNftsLoading(true);
      try {
        const [t20, tnft] = await Promise.all([getTxErc20Transfers(hash), getTxNftTransfers(hash)]);
        if (!cancelled) {
          setErc20(t20.map(({ contract_address, from_address, to_address, value }) => ({ contract_address, from_address, to_address, value })));
          setNfts(tnft.map(({ contract_address, token_id, from_address, to_address }) => ({ contract_address, token_id, from_address, to_address })));
          setErc20Loading(false);
          setNftsLoading(false);
        }
        // fetch token metadata for ERC-20 contracts
        const unique = Array.from(new Set(t20.map(t => t.contract_address.toLowerCase())));
        const updates: Record<string, { symbol: string | null; decimals: number }> = {};
        for (const addr of unique) {
          try {
            const t = await getToken(addr);
            updates[addr] = { symbol: t.symbol, decimals: t.decimals };
          } catch {
            updates[addr] = { symbol: null, decimals: 18 };
          }
        }
        if (!cancelled && Object.keys(updates).length) setTokenMeta(updates);
      } catch {
        if (!cancelled) {
          setErc20([]);
          setNfts([]);
          setErc20Loading(false);
          setNftsLoading(false);
        }
      }
    })();
    return () => { cancelled = true; };
  }, [hash]);

  type DetailRow = { label: string; value: ReactNode; stacked?: boolean };
  const details: DetailRow[] = transaction ? [
    {
      label: 'Transaction Hash',
      stacked: true,
      value: (
        <div className="flex items-center space-x-2">
          <span className="hash text-gray-200">{transaction.hash}</span>
          <CopyButton text={transaction.hash} />
        </div>
      ),
    },
    { label: 'Status', value: <StatusBadge status={transaction.status} /> },
    { label: 'Block', value: <BlockLink blockNumber={transaction.block_number} /> },
    { label: 'Timestamp', value: formatTimestamp(transaction.timestamp) },
    {
      label: 'From',
      stacked: true,
      value: (
        <div className="flex items-center space-x-2">
          <AddressLink address={transaction.from_address} truncate={false} />
          <CopyButton text={transaction.from_address} />
        </div>
      ),
    },
    {
      label: 'To',
      stacked: true,
      value: transaction.to_address ? (
        <div className="flex items-center space-x-2">
          <AddressLink address={transaction.to_address} truncate={false} />
          <CopyButton text={transaction.to_address} />
        </div>
      ) : transaction.contract_created ? (
        <div className="flex items-center space-x-2">
          <span className="text-gray-500 mr-2">[Contract Created]</span>
          <AddressLink address={transaction.contract_created} truncate={false} />
          <CopyButton text={transaction.contract_created} />
        </div>
      ) : (
        <span className="text-gray-400">N/A</span>
      ),
    },
    { label: 'Value', value: `${formatEtherExact(transaction.value)} ETH` },
    { label: 'Transaction Fee', value: `${formatEtherExact((BigInt(transaction.gas_used) * BigInt(transaction.gas_price)).toString())} ETH` },
    { label: 'Gas Price', value: formatGasPrice(transaction.gas_price) },
    { label: 'Gas Used', value: formatGas(transaction.gas_used.toString()) },
    { label: 'Block Index', value: transaction.block_index.toString() },
  ] : [
    { label: 'Transaction Hash', value: hash ? truncateHash(hash, 20, 20) : '---', stacked: true },
    { label: 'Status', value: '---' },
    { label: 'Block', value: '---' },
    { label: 'Timestamp', value: '---' },
    { label: 'From', value: '---', stacked: true },
    { label: 'To', value: '---', stacked: true },
    { label: 'Value', value: '---' },
    { label: 'Transaction Fee', value: '---' },
    { label: 'Gas Price', value: '---' },
    { label: 'Gas Used', value: '---' },
    { label: 'Block Index', value: '---' },
  ];

  return (
    <div>
      {!txLoading && !transaction && (
        <div className="card p-4 mb-6">
          <p className="text-gray-200 font-medium">This transaction does not exist.</p>
          {txError?.error && <p className="text-gray-500 text-sm mt-1">{txError.error}</p>}
        </div>
      )}
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-fg">Transaction</h1>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        <aside className="lg:col-span-3">
          <div className="card p-3">
            <h2 className="text-base font-semibold text-fg mb-3">Overview</h2>
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

        <section className="lg:col-span-9 space-y-6">
          <div className="card p-3">
            <div className="flex items-center justify-between mb-3">
              <div className="flex gap-2">
                <button className={`px-3 py-1.5 rounded-full text-sm ${subTab === 'tokens' ? 'bg-dark-700/70 text-fg' : 'text-gray-400 hover:text-fg hover:bg-dark-700/40'}`} onClick={() => setSubTab('tokens')}>
                  Tokens <span className="text-gray-400">({formatNumber(tokensCount)})</span>
                </button>
                <button className={`px-3 py-1.5 rounded-full text-sm ${subTab === 'nfts' ? 'bg-dark-700/70 text-fg' : 'text-gray-400 hover:text-fg hover:bg-dark-700/40'}`} onClick={() => setSubTab('nfts')}>
                  NFTs <span className="text-gray-400">({formatNumber(nftsCount)})</span>
                </button>
                <button className={`px-3 py-1.5 rounded-full text-sm ${subTab === 'logs' ? 'bg-dark-700/70 text-fg' : 'text-gray-400 hover:text-fg hover:bg-dark-700/40'}`} onClick={() => setSubTab('logs')}>
                  Logs <span className="text-gray-400">({formatNumber(logsCount)})</span>
                </button>
              </div>
              {subTab === 'logs' && logsCount > 0 && (
                <span className="text-gray-400 text-xs">{formatNumber(logsCount)} events</span>
              )}
            </div>

            {subTab === 'logs' ? (
              <EventLogs
                logs={logs}
                pagination={logsPagination}
                onPageChange={setLogsPage}
                showAddress={true}
                showTxHash={false}
                loading={logsLoading}
              />
            ) : subTab === 'tokens' ? (
              <div className="overflow-x-auto">
                {erc20Loading ? (
                  <div className="py-8"><Loading size="sm" /></div>
                ) : erc20.length === 0 ? (
                  <p className="text-gray-400 text-sm">No token transfers detected in this transaction.</p>
                ) : (
                  <table className="w-full">
                    <thead>
                      <tr className="bg-dark-700">
                        <th className="table-cell text-left table-header">Token</th>
                        <th className="table-cell text-left table-header">Contract</th>
                        <th className="table-cell text-left table-header">From</th>
                        <th className="table-cell text-left table-header">To</th>
                        <th className="table-cell text-right table-header">Amount</th>
                      </tr>
                    </thead>
                    <tbody>
                      {erc20.map((t, idx) => {
                        const meta = tokenMeta[t.contract_address.toLowerCase()];
                        const decimals = meta?.decimals ?? 18;
                        const symbol = meta?.symbol ?? '';
                        return (
                          <tr key={`${t.contract_address}-${idx}`} className="hover:bg-dark-700/50 transition-colors">
                            <td className="table-cell text-fg text-sm">{symbol || 'ERC-20'}</td>
                            <td className="table-cell"><AddressLink address={t.contract_address} /></td>
                            <td className="table-cell"><AddressLink address={t.from_address} /></td>
                            <td className="table-cell"><AddressLink address={t.to_address} /></td>
                            <td className="table-cell text-right font-mono text-xs text-gray-200">{formatTokenAmountExact(t.value, decimals)} {symbol}</td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                )}
              </div>
            ) : (
              <div className="overflow-x-auto">
                {nftsLoading ? (
                  <div className="py-8"><Loading size="sm" /></div>
                ) : nfts.length === 0 ? (
                  <p className="text-gray-400 text-sm">No NFT transfers detected in this transaction.</p>
                ) : (
                  <table className="w-full">
                    <thead>
                      <tr className="bg-dark-700">
                        <th className="table-cell text-left table-header">Collection</th>
                        <th className="table-cell text-left table-header">Token ID</th>
                        <th className="table-cell text-left table-header">From</th>
                        <th className="table-cell text-left table-header">To</th>
                      </tr>
                    </thead>
                    <tbody>
                      {nfts.map((t, idx) => (
                        <tr key={`${t.contract_address}-${t.token_id}-${idx}`} className="hover:bg-dark-700/50 transition-colors">
                          <td className="table-cell"><AddressLink address={t.contract_address} /></td>
                          <td className="table-cell">
                            <a href={`/nfts/${t.contract_address}/${t.token_id}`} className="text-accent-primary hover:underline">#{truncateHash(t.token_id, 10, 6)}</a>
                          </td>
                          <td className="table-cell"><AddressLink address={t.from_address} /></td>
                          <td className="table-cell"><AddressLink address={t.to_address} /></td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>
            )}
          </div>

          {transaction?.input_data && transaction.input_data !== '0x' && (
            <div className="card p-3">
              <div className="flex items-center justify-between mb-3">
                <h2 className="text-base font-semibold text-fg">Input Data</h2>
                <button
                  className="btn btn-secondary text-xs"
                  onClick={() => setShowInput((v) => !v)}
                  aria-expanded={showInput}
                >
                  {showInput ? 'Hide' : 'Show'}
                </button>
              </div>
              {showInput && (
                <div className="bg-dark-700 p-3 overflow-x-auto">
                  <pre className="hash text-gray-300 text-xs whitespace-pre-wrap break-all">
                    {transaction.input_data}
                  </pre>
                </div>
              )}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
