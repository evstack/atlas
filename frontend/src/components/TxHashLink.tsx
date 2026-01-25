import { Link } from 'react-router-dom';
import { truncateHash } from '../utils';

interface TxHashLinkProps {
  hash: string;
  truncate?: boolean;
  className?: string;
}

export default function TxHashLink({ hash, truncate = true, className = '' }: TxHashLinkProps) {
  const displayHash = truncate ? truncateHash(hash, 10, 8) : hash;

  return (
    <Link
      to={`/tx/${hash}`}
      className={`address ${className}`}
      title={hash}
    >
      {displayHash}
    </Link>
  );
}
