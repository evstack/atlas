import type { ApiError } from '../types';

interface ErrorProps {
  error: ApiError | null;
  onRetry?: () => void;
}

export default function Error({ error, onRetry }: ErrorProps) {
  if (!error) return null;

  return (
    <div className="card border-l-4 border-l-accent-error">
      <div className="flex items-start justify-between">
        <div>
          <h3 className="text-accent-error font-medium">Error</h3>
          <p className="text-gray-400 text-sm mt-1">{error.error}</p>
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
