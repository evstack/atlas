import { Link } from 'react-router-dom';
import { formatNumber } from '../utils';

interface BlockLinkProps {
  blockNumber: number;
  className?: string;
}

export default function BlockLink({ blockNumber, className = '' }: BlockLinkProps) {
  return (
    <Link
      to={`/blocks/${blockNumber}`}
      className={`address ${className}`}
    >
      {formatNumber(blockNumber)}
    </Link>
  );
}
