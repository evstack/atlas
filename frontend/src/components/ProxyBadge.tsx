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

function getProxyTypeColor(): string {
  return 'bg-dark-600 text-white border-dark-500';
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
  const typeColor = getProxyTypeColor();

  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      <div className="flex items-center gap-2">
        <span className={`px-2 py-1 rounded border text-xs font-medium ${typeColor}`}>
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

  const typeColor = getProxyTypeColor();

  return (
    <span
      className={`inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium ${typeColor}`}
      title={`${getProxyTypeLabel(proxyInfo.proxy_type)} Proxy - Implementation: ${proxyInfo.implementation_address}`}
    >
      Proxy
    </span>
  );
}
