import { useEffect, useMemo, useState } from 'react';
import { useSearchParams, Link } from 'react-router-dom';
import { search as apiSearch } from '../api/search';
import type { AnySearchResult } from '../types';
import { formatNumber, truncateHash } from '../utils';
import { AddressLink, BlockLink, TxHashLink, Loading } from '../components';

export default function SearchResultsPage() {
  const [params] = useSearchParams();
  const q = params.get('q')?.trim() ?? '';
  const [results, setResults] = useState<AnySearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (!q) { setResults([]); return; }
    (async () => {
      setLoading(true);
      setError(null);
      try {
        const res = await apiSearch(q);
        if (!cancelled) setResults(res.results || []);
      } catch (e: any) {
        if (!cancelled) setError(e?.error || e?.message || 'Failed to search');
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [q]);

  const groups = useMemo(() => ({
    blocks: results.filter(r => r.type === 'block') as any[],
    transactions: results.filter(r => r.type === 'transaction') as any[],
    addresses: results.filter(r => r.type === 'address') as any[],
    nfts: results.filter(r => r.type === 'nft') as any[],
    nftCollections: results.filter(r => r.type === 'nft_collection') as any[],
  }), [results]);

  if (!q) {
    return (
      <div className="card p-4">
        <p className="text-gray-200">Enter a query to search blocks, transactions, addresses, and NFTs.</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-white">Search</h1>
        <p className="text-gray-400 text-sm">Results for “{q}”</p>
      </div>
      {loading ? (
        <div className="py-10"><Loading size="sm" /></div>
      ) : error ? (
        <div className="card p-4"><p className="text-red-400 text-sm">{error}</p></div>
      ) : results.length === 0 ? (
        <div className="card p-4"><p className="text-gray-400 text-sm">No results found.</p></div>
      ) : (
        <>
          <Section title={`Blocks (${formatNumber(groups.blocks.length)})`}>
            <table className="w-full">
              <thead><tr className="bg-dark-700"><th className="table-cell text-left table-header">Block</th><th className="table-cell text-left table-header">Hash</th><th className="table-cell text-left table-header">Txns</th></tr></thead>
              <tbody>
                {groups.blocks.map((b: any) => (
                  <tr key={b.hash} className="hover:bg-dark-700/50 transition-colors">
                    <td className="table-cell"><BlockLink blockNumber={b.number} /></td>
                    <td className="table-cell"><span className="hash text-gray-300 text-xs">{truncateHash(b.hash, 10, 8)}</span></td>
                    <td className="table-cell text-xs text-gray-300">{b.transaction_count}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </Section>

          <Section title={`Transactions (${formatNumber(groups.transactions.length)})`}>
            <table className="w-full">
              <thead><tr className="bg-dark-700"><th className="table-cell text-left table-header">Tx Hash</th><th className="table-cell text-left table-header">Block</th><th className="table-cell text-left table-header">From</th><th className="table-cell text-left table-header">To</th></tr></thead>
              <tbody>
                {groups.transactions.map((t: any) => (
                  <tr key={t.hash} className="hover:bg-dark-700/50 transition-colors">
                    <td className="table-cell"><TxHashLink hash={t.hash} /></td>
                    <td className="table-cell"><BlockLink blockNumber={t.block_number} /></td>
                    <td className="table-cell"><AddressLink address={t.from_address} /></td>
                    <td className="table-cell">{t.to_address ? <AddressLink address={t.to_address} /> : <span className="text-gray-500 text-xs">Contract Creation</span>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </Section>

          <Section title={`Addresses (${formatNumber(groups.addresses.length)})`}>
            <ul className="divide-y divide-dark-700">
              {groups.addresses.map((a: any) => (
                <li key={a.address} className="py-2">
                  <AddressLink address={a.address} />
                  <span className="ml-2 text-xs text-gray-500">{a.is_contract ? 'Contract' : 'EOA'}</span>
                </li>
              ))}
            </ul>
          </Section>

          <Section title={`NFT Collections (${formatNumber(groups.nftCollections.length)})`}>
            <ul className="divide-y divide-dark-700">
              {groups.nftCollections.map((c: any) => (
                <li key={c.address} className="py-2 flex items-center justify-between">
                  <Link to={`/nfts/${c.address}`} className="text-accent-primary hover:underline">
                    {c.name || 'NFT Collection'}
                  </Link>
                  <span className="text-xs text-gray-500">{c.address}</span>
                </li>
              ))}
            </ul>
          </Section>

          <Section title={`NFTs (${formatNumber(groups.nfts.length)})`}>
            <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-4">
              {groups.nfts.map((n: any) => (
                <Link key={`${n.contract_address}-${n.token_id}`} to={`/nfts/${n.contract_address}/${n.token_id}`} className="block group">
                  <div className="aspect-square bg-dark-700 border border-dark-600 rounded-lg overflow-hidden flex items-center justify-center">
                    <span className="text-gray-500 text-xs">{n.name || 'NFT'}</span>
                  </div>
                  <div className="mt-2">
                    <div className="text-white text-sm truncate">#{n.token_id}</div>
                    <div className="text-gray-500 text-xs truncate">{n.contract_address}</div>
                  </div>
                </Link>
              ))}
            </div>
          </Section>
        </>
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="card p-3">
      <h2 className="text-base font-semibold text-white mb-3">{title}</h2>
      {children}
    </section>
  );
}
