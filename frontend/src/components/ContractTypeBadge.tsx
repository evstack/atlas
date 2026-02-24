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
  return (
    <span className={`badge-chip uppercase tracking-wide ${className}`}>
      {label}
    </span>
  );
}
