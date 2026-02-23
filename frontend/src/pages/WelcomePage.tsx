import SearchBar from '../components/SearchBar';
import logoImg from '../assets/logo.png';
import useStats from '../hooks/useStats';
import { formatNumber } from '../utils';
import { useContext, useMemo } from 'react';
import { BlockStatsContext } from '../context/BlockStatsContext';

export default function WelcomePage() {
  const { totals, dailyTx, avgBlockTimeSec, loading } = useStats();
  const { bps } = useContext(BlockStatsContext);
  const headerAvgSec = useMemo(() => (bps && bps > 0 ? 1 / bps : null), [bps]);
  return (
    <div className="min-h-[70vh] flex items-center justify-center">
      <div className="w-full max-w-2xl px-4 text-center">
        <div className="flex justify-center mb-6">
          <img src={logoImg} alt="Atlas" className="h-40 md:h-56 lg:h-64 w-auto rounded-lg" />
        </div>
        <SearchBar />
        <p className="mt-4 text-fg-subtle text-sm">
          Search blocks, transactions, tokens, accounts, and NFTs.
        </p>
        <div className="mt-8 grid grid-cols-2 sm:grid-cols-3 gap-3 text-left">
          <div className="card p-3">
            <p className="text-fg-subtle text-xs mb-1">Total Blocks</p>
            <p className="text-fg text-lg font-semibold">{totals ? formatNumber(totals.blocksTotal) : (loading ? '…' : '—')}</p>
          </div>
          <div className="card p-3">
            <p className="text-fg-subtle text-xs mb-1">Avg Block Time</p>
            <p className="text-fg text-lg font-semibold">
              {headerAvgSec != null
                ? (headerAvgSec < 1 ? `${Math.round(headerAvgSec * 1000)} ms` : `${headerAvgSec.toFixed(1)} s`)
                : (avgBlockTimeSec != null
                    ? (avgBlockTimeSec < 1 ? `${Math.round(avgBlockTimeSec * 1000)} ms` : `${avgBlockTimeSec.toFixed(1)} s`)
                    : (loading ? '…' : '—'))}
            </p>
          </div>
          <div className="card p-3">
            <p className="text-fg-subtle text-xs mb-1">Total Transactions</p>
            <p className="text-fg text-lg font-semibold">{totals ? formatNumber(totals.transactionsTotal) : (loading ? '…' : '—')}</p>
          </div>
          <div className="card p-3">
            <p className="text-fg-subtle text-xs mb-1">Total Addresses</p>
            <p className="text-fg text-lg font-semibold">{totals ? formatNumber(totals.addressesTotal) : (loading ? '…' : '—')}</p>
          </div>
          <div className="card p-3 col-span-2 sm:col-span-1">
            <p className="text-fg-subtle text-xs mb-1">Transactions (24h)</p>
            <p className="text-fg text-lg font-semibold">{dailyTx != null ? formatNumber(dailyTx) : (loading ? '…' : '—')}</p>
          </div>
        </div>
      </div>
    </div>
  );
}
