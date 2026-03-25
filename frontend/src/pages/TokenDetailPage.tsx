import React, { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import {
  AreaChart, Area, BarChart, Bar,
  XAxis, YAxis, Tooltip, ResponsiveContainer,
} from 'recharts';
import { useToken, useTokenHolders, useTokenTransfers } from '../hooks';
import { useTokenChart } from '../hooks/useTokenChart';
import { Pagination, AddressLink, TxHashLink, CopyButton } from '../components';
import Loading from '../components/Loading';
import { formatNumber, formatTokenAmount, formatPercentage, formatTimeAgo, truncateHash } from '../utils';
import { type ChartWindow } from '../api/chartData';

const CHART_ACCENT = '#dc2626';
const CHART_GRID = '#22222e';
const CHART_AXIS_TEXT = '#94a3b8';
const CHART_TOOLTIP_BG = '#0c0c10';

const WINDOWS: { label: string; value: ChartWindow }[] = [
  { label: '1H', value: '1h' },
  { label: '6H', value: '6h' },
  { label: '24H', value: '24h' },
  { label: '7D', value: '7d' },
  { label: '1M', value: '1m' },
  { label: '6M', value: '6m' },
  { label: '1Y', value: '1y' },
];

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n % 1 === 0 ? String(n) : n.toFixed(2);
}

function formatBucketTick(bucket: string, window: ChartWindow): string {
  const d = new Date(bucket);
  if (isNaN(d.getTime())) return bucket;
  if (window === '1m') {
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }
  if (window === '6m' || window === '1y') {
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }
  if (window === '7d') {
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }
  return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
}

function TokenChartCard({ title, children, loading }: { title: string; children: React.ReactNode; loading?: boolean }) {
  return (
    <div className="bg-dark-700/60 border border-dark-600 rounded-xl p-4 relative">
      <p className="text-fg-subtle text-xs uppercase tracking-wide mb-3">{title}</p>
      {children}
      {loading && (
        <div className="absolute inset-0 rounded-xl bg-dark-900/60 flex items-center justify-center">
          <div className="w-5 h-5 border-2 border-accent-primary border-t-transparent rounded-full animate-spin" />
        </div>
      )}
    </div>
  );
}

const BUCKET_MS: Record<ChartWindow, number> = {
  '1h':  5 * 60 * 1000,
  '6h':  30 * 60 * 1000,
  '24h': 60 * 60 * 1000,
  '7d':  12 * 60 * 60 * 1000,
  '1m':  24 * 60 * 60 * 1000,
  '6m':  7 * 24 * 60 * 60 * 1000,
  '1y':  14 * 24 * 60 * 60 * 1000,
};

function formatBucketTooltip(bucket: string, window: ChartWindow): string {
  const start = new Date(bucket);
  if (isNaN(start.getTime())) return bucket;
  const end = new Date(start.getTime() + BUCKET_MS[window]);

  if (BUCKET_MS[window] < 24 * 60 * 60 * 1000) {
    const date = start.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
    const t0 = start.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
    const t1 = end.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
    return `${date}, ${t0} – ${t1}`;
  }

  const d0 = start.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  const d1 = new Date(end.getTime() - 1).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  return d0 === d1 ? d0 : `${d0} – ${d1}`;
}

type TabType = 'holders' | 'transfers';

export default function TokenDetailPage() {
  const { address } = useParams<{ address: string }>();
  const [activeTab, setActiveTab] = useState<TabType>('holders');
  const [holdersPage, setHoldersPage] = useState(1);
  const [transfersPage, setTransfersPage] = useState(1);
  const [chartWindow, setChartWindow] = useState<ChartWindow>('24h');

  const { token } = useToken(address);
  const { holders, pagination: holdersPagination } = useTokenHolders(address, { page: holdersPage, limit: 20 });
  const { transfers, pagination: transfersPagination } = useTokenTransfers(address, { page: transfersPage, limit: 20 });
  const { data: chartData, loading: chartLoading } = useTokenChart(address, chartWindow);

  const tabs: { id: TabType; label: string; count?: number }[] = [
    { id: 'holders', label: 'Holders', count: holdersPagination?.total },
    { id: 'transfers', label: 'Transfers', count: transfersPagination?.total },
  ];

  return (
    <div>
      {/* Header */}
      <div className="flex items-center space-x-3 mb-6">
        <h1 className="text-2xl font-bold text-fg">
          {token?.name || 'Token'}
        </h1>
        {token?.symbol && (
          <span className="bg-dark-700 px-3 py-1 text-gray-300 text-sm">
            {token.symbol}
          </span>
        )}
      </div>

      {/* Overview Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Contract Address</p>
          <div className="flex items-center space-x-2">
            {address ? (
              <>
                <Link to={`/address/${address}`} className="address text-sm">
                  {truncateHash(address, 8, 6)}
                </Link>
                <CopyButton text={address} />
              </>
            ) : (
              <span className="text-gray-200">---</span>
            )}
          </div>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Decimals</p>
          <p className="text-xl font-semibold text-fg">{token?.decimals ?? '---'}</p>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Total Supply</p>
          <p className="text-xl font-semibold text-fg font-mono">
            {token?.total_supply
              ? `${formatTokenAmount(token.total_supply, token.decimals)} ${token.symbol || ''}`
              : '---'}
          </p>
        </div>
        <div className="card">
          <p className="text-gray-400 text-sm mb-1">Holders</p>
          <p className="text-xl font-semibold text-fg">
            {holdersPagination ? formatNumber(holdersPagination.total) : '---'}
          </p>
        </div>
      </div>

      {/* Activity Charts */}
      <div className="mb-8">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-fg">Token Activity</h2>
          <div className="flex gap-1 bg-dark-700/60 border border-dark-600 rounded-lg p-1">
            {WINDOWS.map(({ label, value: w }) => (
              <button
                key={w}
                onClick={() => setChartWindow(w)}
                className={`px-3 py-1 text-xs rounded-md transition-colors ${
                  chartWindow === w ? 'bg-accent-primary text-white' : 'text-fg-subtle hover:text-fg'
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        {chartLoading && chartData.length === 0 ? (
          <div className="py-10"><Loading text="Fetching chart data" /></div>
        ) : (
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {/* Transfer Count */}
            <TokenChartCard title="Transfers" loading={chartLoading}>
              <ResponsiveContainer width="100%" height={200}>
                <BarChart data={chartData} margin={{ top: 4, right: 30, left: 0, bottom: 0 }}>
                  <XAxis
                    dataKey="bucket"
                    tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }}
                    tickFormatter={(v) => formatBucketTick(v, chartWindow)}
                    interval="preserveStartEnd"
                  />
                  <YAxis tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }} tickFormatter={formatCompact} width={40} />
                  <Tooltip
                    contentStyle={{ background: CHART_TOOLTIP_BG, border: `1px solid ${CHART_GRID}`, borderRadius: 8 }}
                    labelStyle={{ color: CHART_AXIS_TEXT }}
                    itemStyle={{ color: '#f8fafc' }}
                    formatter={(v: number) => [formatCompact(v), 'Transfers']}
                    labelFormatter={(v) => formatBucketTooltip(v, chartWindow)}
                  />
                  <Bar dataKey="transfer_count" fill={CHART_ACCENT} radius={[2, 2, 0, 0]} isAnimationActive={false} />
                </BarChart>
              </ResponsiveContainer>
            </TokenChartCard>

            {/* Transfer Volume */}
            <TokenChartCard title={`Volume (${token?.symbol || 'tokens'})`} loading={chartLoading}>
              <ResponsiveContainer width="100%" height={200}>
                <AreaChart data={chartData} margin={{ top: 4, right: 30, left: 0, bottom: 0 }}>
                  <defs>
                    <linearGradient id="tokenVolumeGradient" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="5%" stopColor={CHART_ACCENT} stopOpacity={0.3} />
                      <stop offset="95%" stopColor={CHART_ACCENT} stopOpacity={0} />
                    </linearGradient>
                  </defs>
                  <XAxis
                    dataKey="bucket"
                    tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }}
                    tickFormatter={(v) => formatBucketTick(v, chartWindow)}
                    interval="preserveStartEnd"
                  />
                  <YAxis tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }} tickFormatter={formatCompact} width={40} />
                  <Tooltip
                    contentStyle={{ background: CHART_TOOLTIP_BG, border: `1px solid ${CHART_GRID}`, borderRadius: 8 }}
                    labelStyle={{ color: CHART_AXIS_TEXT }}
                    itemStyle={{ color: '#f8fafc' }}
                    formatter={(v: number) => [formatCompact(v), `Volume (${token?.symbol || 'tokens'})`]}
                    labelFormatter={(v) => formatBucketTooltip(v, chartWindow)}
                  />
                  <Area
                    type="linear"
                    dataKey="volume"
                    stroke={CHART_ACCENT}
                    fill="url(#tokenVolumeGradient)"
                    strokeWidth={2}
                    dot={false}
                    isAnimationActive={false}
                  />
                </AreaChart>
              </ResponsiveContainer>
            </TokenChartCard>
          </div>
        )}
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
              {tab.count !== undefined && tab.count > 0 && (
                <span className="ml-2 text-gray-500">({formatNumber(tab.count)})</span>
              )}
            </button>
          ))}
        </nav>
      </div>

      {/* Tab Content */}
      {activeTab === 'holders' && (
        <div className="card overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="bg-dark-700">
                  <th className="table-cell text-left table-header">Rank</th>
                  <th className="table-cell text-left table-header">Address</th>
                  <th className="table-cell text-right table-header">Balance</th>
                  <th className="table-cell text-right table-header">Percentage</th>
                </tr>
              </thead>
              <tbody>
                {holders.map((holder, index) => (
                  <tr key={holder.address} className="hover:bg-dark-700/50 transition-colors">
                    <td className="table-cell text-gray-400">
                      {(holdersPagination ? (holdersPagination.page - 1) * holdersPagination.limit : 0) + index + 1}
                    </td>
                    <td className="table-cell">
                      <AddressLink address={holder.address} />
                    </td>
                    <td className="table-cell text-right font-mono text-gray-200">
                      {formatTokenAmount(holder.balance, token?.decimals ?? 18)} {token?.symbol || ''}
                    </td>
                    <td className="table-cell text-right text-gray-300">
                      {formatPercentage(holder.percentage)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {holdersPagination && holdersPagination.total_pages > 1 && (
            <Pagination
              currentPage={holdersPagination.page}
              totalPages={holdersPagination.total_pages}
              onPageChange={setHoldersPage}
            />
          )}
        </div>
      )}

      {activeTab === 'transfers' && (
        <div className="card overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="bg-dark-700">
                  <th className="table-cell text-left table-header">Tx Hash</th>
                  <th className="table-cell text-left table-header">Age</th>
                  <th className="table-cell text-left table-header">From</th>
                  <th className="table-cell text-left table-header">To</th>
                  <th className="table-cell text-right table-header">Amount</th>
                </tr>
              </thead>
              <tbody>
                {transfers.map((transfer) => (
                  <tr key={`${transfer.tx_hash}-${transfer.log_index}`} className="hover:bg-dark-700/50 transition-colors">
                    <td className="table-cell">
                      <TxHashLink hash={transfer.tx_hash} />
                    </td>
                    <td className="table-cell text-gray-400 text-sm">
                      {formatTimeAgo(transfer.timestamp)}
                    </td>
                    <td className="table-cell">
                      <AddressLink address={transfer.from_address} />
                    </td>
                    <td className="table-cell">
                      <AddressLink address={transfer.to_address} />
                    </td>
                    <td className="table-cell text-right font-mono text-gray-200">
                      {formatTokenAmount(transfer.value, token?.decimals ?? 18)} {token?.symbol || ''}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {transfersPagination && transfersPagination.total_pages > 1 && (
            <Pagination
              currentPage={transfersPagination.page}
              totalPages={transfersPagination.total_pages}
              onPageChange={setTransfersPage}
            />
          )}
        </div>
      )}
    </div>
  );
}
