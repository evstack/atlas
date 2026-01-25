import { useState } from 'react';
import { useParams } from 'react-router-dom';
import { useTransaction, useTransactionDecodedLogs } from '../hooks';
import { AddressLink, BlockLink, StatusBadge, CopyButton, EventLogs } from '../components';
import { formatTimestamp, formatEther, formatGas, formatGasPrice, formatNumber, truncateHash } from '../utils';

export default function TransactionDetailPage() {
  const { hash } = useParams<{ hash: string }>();
  const [logsPage, setLogsPage] = useState(1);
  const { transaction } = useTransaction(hash);
  const { logs, pagination: logsPagination, loading: logsLoading } = useTransactionDecodedLogs(hash, { page: logsPage, limit: 50 });

  const details = transaction ? [
    {
      label: 'Transaction Hash',
      value: (
        <div className="flex items-center space-x-2">
          <span className="hash text-gray-200">{transaction.hash}</span>
          <CopyButton text={transaction.hash} />
        </div>
      ),
    },
    {
      label: 'Status',
      value: <StatusBadge status={transaction.status} />,
    },
    {
      label: 'Block',
      value: <BlockLink blockNumber={transaction.block_number} />,
    },
    { label: 'Timestamp', value: formatTimestamp(transaction.timestamp) },
    {
      label: 'From',
      value: (
        <div className="flex items-center space-x-2">
          <AddressLink address={transaction.from_address} truncate={false} />
          <CopyButton text={transaction.from_address} />
        </div>
      ),
    },
    {
      label: 'To',
      value: transaction.to_address ? (
        <div className="flex items-center space-x-2">
          <AddressLink address={transaction.to_address} truncate={false} />
          <CopyButton text={transaction.to_address} />
        </div>
      ) : transaction.contract_created ? (
        <div className="flex items-center space-x-2">
          <span className="text-gray-500 mr-2">[Contract Created]</span>
          <AddressLink address={transaction.contract_created} truncate={false} />
          <CopyButton text={transaction.contract_created} />
        </div>
      ) : (
        <span className="text-gray-400">N/A</span>
      ),
    },
    { label: 'Value', value: `${formatEther(transaction.value)} ETH` },
    { label: 'Transaction Fee', value: `${formatEther((BigInt(transaction.gas_used) * BigInt(transaction.gas_price)).toString())} ETH` },
    { label: 'Gas Price', value: formatGasPrice(transaction.gas_price) },
    { label: 'Gas Used', value: formatGas(transaction.gas_used.toString()) },
    { label: 'Block Index', value: transaction.block_index.toString() },
  ] : [
    { label: 'Transaction Hash', value: hash ? truncateHash(hash, 20, 20) : '---' },
    { label: 'Status', value: '---' },
    { label: 'Block', value: '---' },
    { label: 'Timestamp', value: '---' },
    { label: 'From', value: '---' },
    { label: 'To', value: '---' },
    { label: 'Value', value: '---' },
    { label: 'Transaction Fee', value: '---' },
    { label: 'Gas Price', value: '---' },
    { label: 'Gas Used', value: '---' },
    { label: 'Block Index', value: '---' },
  ];

  return (
    <div>
      <h1 className="text-2xl font-bold text-white mb-6">Transaction Details</h1>

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

      {transaction?.input_data && transaction.input_data !== '0x' && (
        <div className="card mt-6">
          <h2 className="text-lg font-semibold text-white mb-4">Input Data</h2>
          <div className="bg-dark-700 p-4 overflow-x-auto">
            <pre className="hash text-gray-300 text-xs whitespace-pre-wrap break-all">
              {transaction.input_data}
            </pre>
          </div>
        </div>
      )}

      {/* Event Logs */}
      <div className="card mt-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-white">Event Logs</h2>
          {logsPagination && logsPagination.total > 0 && (
            <span className="text-gray-400 text-sm">
              {formatNumber(logsPagination.total)} events
            </span>
          )}
        </div>
        <EventLogs
          logs={logs}
          pagination={logsPagination}
          onPageChange={setLogsPage}
          showAddress={true}
          showTxHash={false}
          loading={logsLoading}
        />
      </div>
    </div>
  );
}
