import { useContext, useEffect, useMemo, useState } from 'react';
import SearchBar from '../components/SearchBar';
import useStats from '../hooks/useStats';
import { formatNumber } from '../utils';
import { BlockStatsContext } from '../context/BlockStatsContext';
import { useBranding } from '../hooks/useBranding';
import { getDefaultLogo } from '../assets/defaultLogos';
import { useTheme } from '../hooks/useTheme';
import { StatCard } from '../components';
import { getBlockByNumber } from '../api/blocks';

export default function WelcomePage() {
  const { totals, dailyTx, avgBlockTimeSec, loading } = useStats();
  const { bps, height } = useContext(BlockStatsContext);
  const headerAvgSec = useMemo(() => (bps && bps > 0 ? 1 / bps : null), [bps]);
  const { logoUrl, chainName } = useBranding();
  const { theme } = useTheme();
  const logoSrc = logoUrl || getDefaultLogo(theme);

  const [chainAge, setChainAge] = useState<string | null>(null);
  useEffect(() => {
    getBlockByNumber(1).then((block) => {
      if (!block.timestamp) return;
      const now = Math.floor(Date.now() / 1000);
      const seconds = now - block.timestamp;
      const days = Math.floor(seconds / 86400);
      setChainAge(`${days} days`);
    }).catch(() => {});
  }, []);

  const stats = [
    {
      label: 'Latest block',
      value: height != null ? formatNumber(height) : loading ? '…' : '—',
    },
    {
      label: 'Chain age',
      value: chainAge ?? (loading ? '…' : '—'),
    },
    {
      label: 'Avg block time',
      value:
        headerAvgSec != null
          ? headerAvgSec < 1
            ? `${Math.round(headerAvgSec * 1000)} ms`
            : `${headerAvgSec.toFixed(1)} s`
          : avgBlockTimeSec != null
            ? avgBlockTimeSec < 1
              ? `${Math.round(avgBlockTimeSec * 1000)} ms`
              : `${avgBlockTimeSec.toFixed(1)} s`
            : loading
              ? '…'
              : '—',
    },
    {
      label: 'Transactions',
      value: totals ? formatNumber(totals.transactionsTotal) : loading ? '…' : '—',
    },
    {
      label: 'Active addresses',
      value: totals ? formatNumber(totals.addressesTotal) : loading ? '…' : '—',
    },
    {
      label: 'Transactions / 24h',
      value: dailyTx != null ? formatNumber(dailyTx) : loading ? '…' : '—',
    },
  ];

  return (
    <div className="space-y-8 fade-in-up">
      <div className="flex flex-col items-center gap-6 py-8">
        <img src={logoSrc} alt={chainName} className="h-16 w-auto" />
        <div className="w-full max-w-2xl">
          <SearchBar variant="hero" />
        </div>
      </div>

      <section className="flex justify-center">
        <div className="grid gap-3 w-full max-w-3xl grid-cols-2 md:grid-cols-3">
          {stats.map((stat) => (
            <StatCard
              key={stat.label}
              label={stat.label}
              value={stat.value}
              className="!py-3 !px-4"
            />
          ))}
        </div>
      </section>

    </div>
  );
}
