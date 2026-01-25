import { Link } from 'react-router-dom';
import { truncateHash } from '../utils';

interface AddressLinkProps {
  address: string;
  truncate?: boolean;
  className?: string;
}

export default function AddressLink({ address, truncate = true, className = '' }: AddressLinkProps) {
  const displayAddress = truncate ? truncateHash(address, 6, 4) : address;

  return (
    <Link
      to={`/address/${address}`}
      className={`address ${className}`}
      title={address}
    >
      {displayAddress}
    </Link>
  );
}
