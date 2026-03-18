import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { TxHashLink, Loading, Error } from '../components';
import { formatEtherExact, formatNumber, toApiError } from '../utils';
import type { ApiError } from '../types';
import { requestFaucet } from '../api/faucet';
import useFaucetInfo from '../hooks/useFaucetInfo';
import NotFoundPage from './NotFoundPage';

const ADDRESS_RE = /^0x[a-fA-F0-9]{40}$/;

function formatCountdown(totalSeconds: number): string {
  const safeSeconds = Math.max(0, Math.ceil(totalSeconds));
  const minutes = Math.floor(safeSeconds / 60);
  const seconds = safeSeconds % 60;

  if (minutes === 0) {
    return `${seconds}s`;
  }

  if (seconds === 0) {
    return `${minutes}m`;
  }

  return `${minutes}m ${seconds}s`;
}

function isValidAddress(address: string): boolean {
  return ADDRESS_RE.test(address);
}

export default function FaucetPage() {
  const { faucetInfo, loading, error, notFound, refetch } = useFaucetInfo();
  const [address, setAddress] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<ApiError | null>(null);
  const [txHash, setTxHash] = useState<string | null>(null);
  const [cooldownUntil, setCooldownUntil] = useState<number | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [disabled, setDisabled] = useState(false);

  useEffect(() => {
    if (!cooldownUntil) return;

    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [cooldownUntil]);

  useEffect(() => {
    if (!cooldownUntil || now < cooldownUntil) return;

    setCooldownUntil(null);
    setSubmitError((current) => (current?.status === 429 ? null : current));
  }, [cooldownUntil, now]);

  const cooldownRemainingSeconds = cooldownUntil ? Math.max(0, Math.ceil((cooldownUntil - now) / 1000)) : 0;
  const canSubmit = Boolean(faucetInfo) && !submitting && cooldownRemainingSeconds === 0;

  const infoCards = useMemo(() => {
    if (!faucetInfo) return [];

    return [
      {
        label: 'Balance',
        value: `${formatEtherExact(faucetInfo.balance_wei).replace(/^(\d+\.\d{4})\d*$/, '$1')} ETH`,
        hint: 'Current faucet wallet balance',
      },
      {
        label: 'Drip amount',
        value: `${formatEtherExact(faucetInfo.amount_wei)} ETH`,
        hint: 'Per successful request',
      },
      {
        label: 'Cooldown',
        value: faucetInfo.cooldown_minutes === 1 ? '1 minute' : `${formatNumber(faucetInfo.cooldown_minutes)} minutes`,
        hint: 'Per address and per IP',
      },
    ];
  }, [faucetInfo]);

  if (notFound || disabled) {
    return <NotFoundPage />;
  }

  if (loading) {
    return <Loading size="lg" text="Checking faucet availability..." />;
  }

  if (error) {
    return <Error error={error} onRetry={refetch} />;
  }

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    const trimmedAddress = address.trim();
    if (!isValidAddress(trimmedAddress)) {
      setSubmitError({ error: 'Enter a valid EVM address.', status: 400 });
      return;
    }

    if (!canSubmit) {
      if (cooldownRemainingSeconds > 0) {
        setSubmitError({
          error: `Cooling down. Try again in ${formatCountdown(cooldownRemainingSeconds)}.`,
          status: 429,
          retryAfterSeconds: cooldownRemainingSeconds,
        });
      }
      return;
    }

    setSubmitting(true);
    setSubmitError(null);
    setTxHash(null);

    try {
      const response = await requestFaucet(trimmedAddress);
      setTxHash(response.tx_hash);
      setCooldownUntil(null);
      void refetch({ background: true });
    } catch (err: unknown) {
      const apiError = toApiError(err, 'Failed to request faucet funds');
      if (apiError.status === 404) {
        setDisabled(true);
        return;
      }

      if (apiError.status === 429) {
        const retryAfterSeconds = apiError.retryAfterSeconds ?? 60;
        setCooldownUntil(Date.now() + retryAfterSeconds * 1000);
      }

      setSubmitError(apiError);
    } finally {
      setSubmitting(false);
    }
  };

  const cooldownBanner = submitError?.status === 429 ? (
    <div className="card border-l-4 border-l-amber-400">
      <p className="text-amber-300 font-medium">Rate limited</p>
      <p className="text-gray-400 text-sm mt-1">{submitError.error}</p>
      <p className="text-gray-200 text-sm mt-2">
        Try again in <span className="font-mono">{formatCountdown(cooldownRemainingSeconds || (submitError.retryAfterSeconds ?? 0))}</span>.
      </p>
    </div>
  ) : submitError ? (
    <div className="card border-l-4 border-l-accent-error">
      <p className="text-accent-error font-medium">Request failed</p>
      <p className="text-gray-400 text-sm mt-1">{submitError.error}</p>
    </div>
  ) : null;

  return (
    <div className="relative">
      <div className="absolute -top-20 right-0 h-64 w-64 rounded-full bg-accent-primary/10 blur-3xl pointer-events-none" />
      <div className="absolute -bottom-24 left-0 h-72 w-72 rounded-full bg-red-500/10 blur-3xl pointer-events-none" />

      <div className="relative max-w-2xl mx-auto space-y-6">
        <div className="card p-6 md:p-8">
          <div className="flex items-start justify-between gap-4">
            <div>
              <p className="text-xs uppercase tracking-[0.28em] text-gray-500">Faucet</p>
              <h1 className="mt-2 text-3xl font-bold text-fg">Request test ETH</h1>
              <p className="mt-3 text-sm leading-6 text-gray-400">
                Drips are rate-limited per address and per IP. Use this faucet for test networks only.
              </p>
            </div>
          </div>

          <form className="mt-6 space-y-4" onSubmit={handleSubmit}>
            <label className="block">
              <span className="text-sm font-medium text-fg">Destination address</span>
              <input
                type="text"
                value={address}
                onChange={(event) => setAddress(event.target.value)}
                placeholder="0x0000000000000000000000000000000000000000"
                autoComplete="off"
                spellCheck={false}
                className="mt-2 w-full rounded-xl border border-dark-600 bg-dark-900/70 px-4 py-3 text-fg placeholder:text-gray-600 focus:outline-none focus:ring-2 focus:ring-accent-primary/50 focus:border-accent-primary/40 transition-colors"
              />
            </label>

            <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
              <button
                type="submit"
                disabled={!canSubmit}
                className="btn btn-primary disabled:opacity-60 disabled:cursor-not-allowed"
              >
                {submitting
                  ? 'Sending...'
                  : cooldownRemainingSeconds > 0
                    ? `Retry in ${formatCountdown(cooldownRemainingSeconds)}`
                    : 'Request faucet funds'}
              </button>
              <p className="text-xs text-gray-500">
                Enter a checksummed or lowercase EVM address. Empty or malformed inputs are rejected.
              </p>
            </div>
          </form>
        </div>

        <div className="grid gap-4 sm:grid-cols-3">
          {infoCards.map((card) => (
            <div key={card.label} className="card p-4">
              <p className="text-xs uppercase tracking-[0.24em] text-gray-500">{card.label}</p>
              <p className="mt-2 text-lg font-semibold text-fg truncate" title={card.value}>{card.value}</p>
              <p className="mt-1 text-xs text-gray-500">{card.hint}</p>
            </div>
          ))}
        </div>

        {cooldownBanner}

        {txHash && (
          <div className="card p-4">
            <p className="text-xs uppercase tracking-[0.24em] text-gray-500">Transaction sent</p>
            <p className="mt-2 text-sm text-gray-300">
              Faucet transfer broadcast successfully. Track it here:
            </p>
            <div className="mt-3">
              <TxHashLink hash={txHash} truncate={false} />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
