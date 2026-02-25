import { useEffect, useState } from 'react';
import { getChainStatus, type ChainStatusResponse } from '../api/status';
import { formatNumber } from '../utils';
import Loading from '../components/Loading';

export default function StatusPage() {
  const [status, setStatus] = useState<ChainStatusResponse | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

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

  const lastIndexed = status?.indexed_at
    ? new Date(status.indexed_at).toLocaleString(undefined, {
        timeStyle: 'medium',
        dateStyle: 'medium',
      })
    : '—';

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-fg">Status</h1>
      </div>

      <div className="card">
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
            <StatusStat label="Chain ID" value={status ? status.chain_id.toString() : '—'} />
            <StatusStat label="Chain Name" value={status?.chain_name || 'Unknown'} />
            <StatusStat label="Block Height" value={status ? formatNumber(status.block_height) : '—'} />
            <StatusStat label="Total Transactions" value={status ? formatNumber(status.total_transactions) : '—'} />
            <StatusStat label="Total Addresses" value={status ? formatNumber(status.total_addresses) : '—'} />
            <StatusStat label="Last Indexed" value={lastIndexed} />
          </div>
        )}
      </div>
    </div>
  );
}

interface StatusStatProps {
  label: string;
  value: string;
}

function StatusStat({ label, value }: StatusStatProps) {
  return (
    <div className="bg-dark-700/60 border border-dark-600 rounded-xl p-4">
      <p className="text-fg-subtle text-xs uppercase tracking-wide mb-1">{label}</p>
      <p className="text-fg text-lg font-semibold break-words">{value}</p>
    </div>
  );
}
