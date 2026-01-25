import { useParams, Link } from 'react-router-dom';
import { useBlock } from '../hooks';
import { CopyButton } from '../components';
import { formatNumber, formatTimestamp, formatGas, truncateHash } from '../utils';

export default function BlockDetailPage() {
  const { number } = useParams<{ number: string }>();
  const blockNumber = number ? parseInt(number, 10) : undefined;
  const { block } = useBlock(blockNumber);

  const details = block ? [
    { label: 'Block Height', value: formatNumber(block.number) },
    { label: 'Timestamp', value: formatTimestamp(block.timestamp) },
    { label: 'Transactions', value: block.transaction_count.toString() },
    {
      label: 'Block Hash',
      value: (
        <div className="flex items-center space-x-2">
          <span className="hash text-gray-200">{block.hash}</span>
          <CopyButton text={block.hash} />
        </div>
      ),
    },
    {
      label: 'Parent Hash',
      value: (
        <div className="flex items-center space-x-2">
          <Link
            to={`/blocks/${block.number - 1}`}
            className="hash text-accent-primary hover:underline"
          >
            {truncateHash(block.parent_hash, 20, 20)}
          </Link>
          <CopyButton text={block.parent_hash} />
        </div>
      ),
    },
    { label: 'Gas Used', value: formatGas(block.gas_used.toString()) },
    { label: 'Gas Limit', value: formatGas(block.gas_limit.toString()) },
  ] : [
    { label: 'Block Height', value: '---' },
    { label: 'Timestamp', value: '---' },
    { label: 'Transactions', value: '---' },
    { label: 'Block Hash', value: '---' },
    { label: 'Parent Hash', value: '---' },
    { label: 'Gas Used', value: '---' },
    { label: 'Gas Limit', value: '---' },
  ];

  return (
    <div>
      <div className="flex items-center space-x-4 mb-6">
        <h1 className="text-2xl font-bold text-white">
          Block {blockNumber !== undefined ? `#${formatNumber(blockNumber)}` : ''}
        </h1>
        <div className="flex space-x-2">
          {blockNumber !== undefined && blockNumber > 0 && (
            <Link
              to={`/blocks/${blockNumber - 1}`}
              className="btn btn-secondary text-sm"
            >
              Previous
            </Link>
          )}
          {blockNumber !== undefined && (
            <Link
              to={`/blocks/${blockNumber + 1}`}
              className="btn btn-secondary text-sm"
            >
              Next
            </Link>
          )}
        </div>
      </div>

      <div className="card">
        <h2 className="text-lg font-semibold text-white mb-4">Overview</h2>
        <dl className="space-y-4">
          {details.map(({ label, value }) => (
            <div key={label} className="flex flex-col sm:flex-row sm:items-start">
              <dt className="text-gray-400 sm:w-48 flex-shrink-0 mb-1 sm:mb-0">{label}:</dt>
              <dd className="text-gray-200 break-all">{value}</dd>
            </div>
          ))}
        </dl>
      </div>

      {block && block.transaction_count > 0 && (
        <div className="mt-6">
          <Link
            to={`/blocks/${block.number}/transactions`}
            className="btn btn-primary"
          >
            View {block.transaction_count} Transaction{block.transaction_count !== 1 ? 's' : ''}
          </Link>
        </div>
      )}
    </div>
  );
}
