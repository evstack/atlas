import type { ApiError } from '../types';

interface ErrorProps {
  error: ApiError | null;
  onRetry?: () => void;
}

export default function Error({ error, onRetry }: ErrorProps) {
  if (!error) return null;

  return (
    <div className="card border border-accent-error/20 bg-accent-error/5">
      <div className="flex items-start justify-between">
        <div>
          <p className="kicker text-accent-error">Error</p>
          <h3 className="mt-2 font-medium tracking-[-0.03em] text-fg">Request failed</h3>
          <p className="mt-1 text-sm text-fg-subtle">{error.error}</p>
        </div>
        {onRetry && (
          <button
            onClick={onRetry}
            className="btn btn-secondary text-sm"
          >
            Retry
          </button>
        )}
      </div>
    </div>
  );
}
