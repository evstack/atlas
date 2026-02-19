interface Props {
  type?: 'eoa' | 'contract' | 'nft' | 'erc20';
  className?: string;
}

export default function ContractTypeBadge({ type, className = '' }: Props) {
  if (!type) return null;

  const label =
    type === 'erc20' ? 'ERC-20'
    : type === 'nft' ? 'NFT'
    : type === 'contract' ? 'Contract'
    : 'EOA';

  // Use a consistent pill style across types for visual cohesion
  const base = 'bg-dark-600 text-white border-dark-500';

  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded-full border text-xs font-medium ${base} ${className}`}>
      {label}
    </span>
  );
}
