import { useContext, useEffect, useState } from 'react';
import {
  AreaChart,
  Area,
  BarChart,
  Bar,
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import { getChainStatus, type ChainStatusResponse } from '../api/status';
import { type ChartWindow } from '../api/chartData';
import { formatNumber } from '../utils';
import { EntityHeroVisual, Loading, PageHero, SectionPanel, StatCard } from '../components';
import { BlockStatsContext } from '../context/BlockStatsContext';
import { useChartData } from '../hooks/useChartData';
import { useChartColors } from '../hooks/useChartColors';

const WINDOWS: { label: string; value: ChartWindow }[] = [
  { label: '1H', value: '1h' },
  { label: '6H', value: '6h' },
  { label: '24H', value: '24h' },
  { label: '7D', value: '7d' },
  { label: '1M', value: '1m' },
];

export default function StatusPage() {
  const blockStats = useContext(BlockStatsContext);
  const [status, setStatus] = useState<ChainStatusResponse | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [window, setWindow] = useState<ChartWindow>('24h');

  const {
    blocksChart, dailyTxs, gasPriceChart,
    dailyTxsLoading, blocksChartLoading, gasPriceLoading,
    dailyTxsError, blocksChartError, gasPriceError,
  } = useChartData(window);

  const { accent: CHART_ACCENT, grid: CHART_GRID, axisText: CHART_AXIS_TEXT, tooltipBg: CHART_TOOLTIP_BG, tooltipText: CHART_TOOLTIP_TEXT } = useChartColors();

  useEffect(() => {
    let mounted = true;
    const fetchStatus = async () => {
      try {
        setLoading(true);
        setError(null);
        const resp = await getChainStatus();
        if (mounted) {
          setStatus(resp);
        }
      } catch (err) {
        if (mounted) {
          setError(err instanceof Error ? err.message : 'Failed to load status');
        }
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    };

    fetchStatus();
    const id = setInterval(fetchStatus, 5000);
    return () => {
      mounted = false;
      clearInterval(id);
    };
  }, []);

  const liveBlockHeight = blockStats.height ?? status?.block_height ?? null;
  const liveIndexedAt = blockStats.latestBlockEvent?.block.indexed_at ?? status?.indexed_at ?? null;

  const lastIndexed = liveIndexedAt
    ? new Date(liveIndexedAt).toLocaleString(undefined, {
        timeStyle: 'medium',
        dateStyle: 'medium',
      })
    : '—';

  return (
    <div className="space-y-6 fade-in-up">
      <PageHero
        compact
        title="Status"
        visual={<EntityHeroVisual kind="status" />}
      />

      <SectionPanel>
        {loading && !status ? (
          <div className="py-10">
            <Loading text="Fetching status" />
          </div>
        ) : error ? (
          <div className="py-6">
            <p className="text-accent-error text-sm">{error}</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            <StatusStat label="Chain ID" value={status?.chain_id || '—'} />
            <StatusStat label="Chain Name" value={status?.chain_name || 'Unknown'} />
            <StatusStat label="Block Height" value={liveBlockHeight !== null ? formatNumber(liveBlockHeight) : '—'} />
            <StatusStat label="Total Transactions" value={status ? formatNumber(status.total_transactions) : '—'} />
            <StatusStat label="Total Addresses" value={status ? formatNumber(status.total_addresses) : '—'} />
            <StatusStat label="Last Indexed" value={lastIndexed} />
          </div>
        )}
      </SectionPanel>

      <div className="mt-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-fg">Chain Activity</h2>
          <WindowToggle value={window} onChange={setWindow} />
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            <ChartCard title="Daily Transactions (14d)" loading={dailyTxsLoading} error={dailyTxsError}>
              {!dailyTxsLoading && !dailyTxsError && (
                <ResponsiveContainer width="100%" height={200}>
                  <BarChart data={dailyTxs} margin={{ top: 4, right: 30, left: 0, bottom: 0 }}>
                    <XAxis
                      dataKey="day"
                      tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }}
                      tickFormatter={formatDayLabel}
                      interval="preserveStartEnd"
                    />
                    <YAxis
                      tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }}
                      tickFormatter={formatCompact}
                      width={40}
                    />
                    <Tooltip
                      contentStyle={{ background: CHART_TOOLTIP_BG, border: `1px solid ${CHART_GRID}`, borderRadius: 8 }}
                      labelStyle={{ color: CHART_AXIS_TEXT }}
                      itemStyle={{ color: CHART_TOOLTIP_TEXT }}
                      formatter={(v: unknown) => [formatCompact(v as number), 'Transactions']}
                    />
                    <Bar dataKey="tx_count" fill={CHART_ACCENT} radius={[2, 2, 0, 0]} isAnimationActive={false} />
                  </BarChart>
                </ResponsiveContainer>
              )}
            </ChartCard>

            <ChartCard title="Avg Gas Used per Block" loading={blocksChartLoading} error={blocksChartError}>
              {!blocksChartLoading && !blocksChartError && (
                <ResponsiveContainer width="100%" height={200}>
                  <AreaChart data={blocksChart} margin={{ top: 4, right: 30, left: 0, bottom: 0 }}>
                    <defs>
                      <linearGradient id="gasGradient" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor={CHART_ACCENT} stopOpacity={0.3} />
                        <stop offset="95%" stopColor={CHART_ACCENT} stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <XAxis
                      dataKey="bucket"
                      tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }}
                      tickFormatter={(v: string) => formatBucketTick(v, window)}
                      interval="preserveStartEnd"
                    />
                    <YAxis
                      tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }}
                      tickFormatter={formatCompact}
                      width={40}
                    />
                    <Tooltip
                      contentStyle={{ background: CHART_TOOLTIP_BG, border: `1px solid ${CHART_GRID}`, borderRadius: 8 }}
                      labelStyle={{ color: CHART_AXIS_TEXT }}
                      itemStyle={{ color: CHART_TOOLTIP_TEXT }}
                      formatter={(v: unknown) => [formatCompact(v as number), 'Avg Gas Used']}
                      labelFormatter={(v) => formatBucketTooltip(v, window)}
                    />
                    <Area
                      type="linear"
                      dataKey="avg_gas_used"
                      stroke={CHART_ACCENT}
                      fill="url(#gasGradient)"
                      strokeWidth={2}
                      dot={false}
                      isAnimationActive={false}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              )}
            </ChartCard>

            <ChartCard title="Transactions" loading={blocksChartLoading} error={blocksChartError}>
              {!blocksChartLoading && !blocksChartError && (
                <ResponsiveContainer width="100%" height={200}>
                  <BarChart data={blocksChart} margin={{ top: 4, right: 30, left: 0, bottom: 0 }}>
                    <XAxis
                      dataKey="bucket"
                      tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }}
                      tickFormatter={(v: string) => formatBucketTick(v, window)}
                      interval="preserveStartEnd"
                    />
                    <YAxis
                      tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }}
                      tickFormatter={formatCompact}
                      width={40}
                    />
                    <Tooltip
                      contentStyle={{ background: CHART_TOOLTIP_BG, border: `1px solid ${CHART_GRID}`, borderRadius: 8 }}
                      labelStyle={{ color: CHART_AXIS_TEXT }}
                      itemStyle={{ color: CHART_TOOLTIP_TEXT }}
                      formatter={(v: unknown) => [formatCompact(v as number), 'Transactions']}
                      labelFormatter={(v) => formatBucketTooltip(v, window)}
                    />
                    <Bar dataKey="tx_count" fill={CHART_ACCENT} radius={[2, 2, 0, 0]} isAnimationActive={false} />
                  </BarChart>
                </ResponsiveContainer>
              )}
            </ChartCard>

            <ChartCard title="Avg Gas Price" loading={gasPriceLoading} error={gasPriceError}>
              {!gasPriceLoading && !gasPriceError && (
                <ResponsiveContainer width="100%" height={200}>
                  <LineChart data={gasPriceChart} margin={{ top: 4, right: 30, left: 0, bottom: 0 }}>
                    <XAxis
                      dataKey="bucket"
                      tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }}
                      tickFormatter={(v: string) => formatBucketTick(v, window)}
                      interval="preserveStartEnd"
                    />
                    <YAxis
                      tick={{ fill: CHART_AXIS_TEXT, fontSize: 11 }}
                      tickFormatter={formatGwei}
                      width={52}
                    />
                    <Tooltip
                      contentStyle={{ background: CHART_TOOLTIP_BG, border: `1px solid ${CHART_GRID}`, borderRadius: 8 }}
                      labelStyle={{ color: CHART_AXIS_TEXT }}
                      itemStyle={{ color: CHART_TOOLTIP_TEXT }}
                      formatter={(v: unknown) => [formatGwei(v as number | null), 'Avg Gas Price']}
                      labelFormatter={(v) => formatBucketTooltip(v, window)}
                    />
                    <Line
                      type="linear"
                      dataKey="avg_gas_price"
                      stroke={CHART_ACCENT}
                      strokeWidth={2}
                      dot={false}
                      isAnimationActive={false}
                    />
                  </LineChart>
                </ResponsiveContainer>
              )}
            </ChartCard>
          </div>
      </div>
    </div>
  );
}

// ─── sub-components ───────────────────────────────────────────────────────────

interface StatusStatProps {
  label: string;
  value: string;
}

function StatusStat({ label, value }: StatusStatProps) {
  return (
    <StatCard label={label} value={value} />
  );
}

function ChartCard({
  title,
  children,
  loading,
  error,
}: {
  title: string;
  children: React.ReactNode;
  loading?: boolean;
  error?: string | null;
}) {
  return (
    <div className="card p-4">
      <p className="kicker mb-3">{title}</p>
      {loading ? (
        <div className="h-[200px] flex items-center justify-center">
          <div className="w-5 h-5 border-2 border-black border-t-transparent rounded-full animate-spin" />
        </div>
      ) : error ? (
        <div className="h-[200px] flex items-center justify-center text-center">
          <p className="max-w-xs text-sm text-accent-error">{error}</p>
        </div>
      ) : (
        children
      )}
    </div>
  );
}

function WindowToggle({ value, onChange }: { value: ChartWindow; onChange: (w: ChartWindow) => void }) {
  return (
    <div className="section-tabs">
      {WINDOWS.map(({ label, value: w }) => (
        <button
          key={w}
          onClick={() => onChange(w)}
          data-active={value === w}
          className="section-tab"
        >
          {label}
        </button>
      ))}
    </div>
  );
}

// ─── formatting helpers ───────────────────────────────────────────────────────

function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function formatGwei(wei: number | null | undefined): string {
  if (wei === null || wei === undefined || Number.isNaN(wei)) {
    return '—';
  }

  const gwei = wei / 1e9;
  if (gwei >= 1_000) return `${(gwei / 1_000).toFixed(1)}K gwei`;
  if (gwei >= 1) return `${gwei.toFixed(2)} gwei`;
  return `${gwei.toFixed(3)} gwei`;
}

function formatDayLabel(day: string): string {
  // day is "YYYY-MM-DD" — show "MMM D"
  const d = new Date(day + 'T00:00:00');
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
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
