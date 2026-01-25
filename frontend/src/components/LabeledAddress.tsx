import { Link } from 'react-router-dom';
import { useLabel } from '../hooks';
import { truncateHash } from '../utils';

interface LabeledAddressProps {
  address: string;
  truncate?: boolean;
  showLogo?: boolean;
  showTags?: boolean;
  className?: string;
}

export default function LabeledAddress({
  address,
  truncate = true,
  showLogo = true,
  showTags = false,
  className = '',
}: LabeledAddressProps) {
  const { label, loading } = useLabel(address);

  const displayAddress = truncate ? truncateHash(address, 6, 4) : address;

  if (loading) {
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

  if (!label) {
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

  return (
    <Link
      to={`/address/${address}`}
      className={`inline-flex items-center gap-2 group ${className}`}
      title={`${label.name} (${address})`}
    >
      {showLogo && label.logo_url && (
        <img
          src={label.logo_url}
          alt={label.name}
          className="w-5 h-5 rounded-full"
          onError={(e) => {
            (e.target as HTMLImageElement).style.display = 'none';
          }}
        />
      )}
      <span className="text-white group-hover:underline font-medium">
        {label.name}
      </span>
      {showTags && label.tags.length > 0 && (
        <span className="text-xs bg-dark-600 text-gray-400 px-1.5 py-0.5">
          {label.tags[0]}
        </span>
      )}
    </Link>
  );
}
