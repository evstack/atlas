import { Link } from 'react-router-dom';
import { useContractProxy } from '../hooks';
import { truncateHash } from '../utils';

interface ProxyBadgeProps {
  address: string;
  showImplementation?: boolean;
  className?: string;
}

function getProxyTypeLabel(proxyType?: string): string {
  if (!proxyType) return 'Proxy';
  switch (proxyType.toLowerCase()) {
    case 'eip1967':
      return 'EIP-1967';
    case 'eip1167':
      return 'EIP-1167 (Minimal)';
    case 'eip897':
      return 'EIP-897';
    case 'openzeppelin':
      return 'OpenZeppelin';
    case 'gnosis_safe':
      return 'Gnosis Safe';
    case 'diamond':
      return 'Diamond (EIP-2535)';
    default:
      return proxyType;
  }
}

export default function ProxyBadge({
  address,
  showImplementation = true,
  className = '',
}: ProxyBadgeProps) {
  const { proxyInfo, loading } = useContractProxy(address);

  if (loading || !proxyInfo) {
    return null;
  }

  const typeLabel = getProxyTypeLabel(proxyInfo.proxy_type);
  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      <div className="flex items-center gap-2">
        <span className="badge-chip text-[0.65rem]">
          Proxy
        </span>
        <span className="text-gray-400 text-xs">
          {typeLabel}
        </span>
      </div>
      {showImplementation && (
        <div className="flex items-center gap-2 text-sm">
          <span className="text-gray-500">Implementation:</span>
          <Link
            to={`/address/${proxyInfo.implementation_address}`}
            className="address"
          >
            {truncateHash(proxyInfo.implementation_address)}
          </Link>
        </div>
      )}
    </div>
  );
}

interface ProxyIndicatorProps {
  address: string;
}

export function ProxyIndicator({ address }: ProxyIndicatorProps) {
  const { proxyInfo, loading } = useContractProxy(address);

  if (loading || !proxyInfo) {
    return null;
  }

  return (
    <span
      className="badge-chip text-[0.65rem]"
      title={`${getProxyTypeLabel(proxyInfo.proxy_type)} Proxy - Implementation: ${proxyInfo.implementation_address}`}
    >
      Proxy
    </span>
  );
}
