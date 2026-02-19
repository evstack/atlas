import { useState } from 'react';
import { TxHashLink, AddressLink, CopyButton, Pagination } from './index';
import { truncateHash, formatNumber } from '../utils';
import type { EventLog, DecodedEventLog, DecodedParam } from '../types';

interface EventLogsProps {
  logs?: EventLog[] | DecodedEventLog[];
  pagination?: { page: number; limit: number; total: number; total_pages: number } | null;
  onPageChange?: (page: number) => void;
  showTxHash?: boolean;
  showAddress?: boolean;
  loading?: boolean;
}

function isDecodedLog(log: EventLog | DecodedEventLog): log is DecodedEventLog {
  return 'event_name' in log;
}

function DecodedParamDisplay({ param }: { param: DecodedParam }) {
  const isAddress = param.type === 'address';

  return (
    <div className="flex items-start gap-2 text-sm">
      <span className="text-gray-500 min-w-[100px]">
        {param.name}
        {param.indexed && (
          <span className="ml-1 text-xs text-gray-500">(indexed)</span>
        )}
      </span>
      <span className="text-gray-400 min-w-[80px]">{param.type}</span>
      <span className="text-gray-200 font-mono break-all">
        {isAddress ? (
          <AddressLink address={param.value} />
        ) : (
          param.value
        )}
      </span>
    </div>
  );
}

function LogCard({ log, showTxHash, showAddress }: { log: EventLog | DecodedEventLog; showTxHash?: boolean; showAddress?: boolean }) {
  const [expanded, setExpanded] = useState(false);
  const decoded = isDecodedLog(log) ? log : null;

  return (
    <div className="border border-dark-600 p-4 rounded-lg hover:border-dark-500 transition-colors">
      {/* Header */}
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-4">
          <span className="text-gray-500 text-sm">#{log.log_index}</span>
          {decoded?.event_name ? (
            <span className="text-white text-sm font-medium">
              {decoded.event_name}
            </span>
          ) : (
            <span className="text-gray-500 text-sm">
              Unknown Event
            </span>
          )}
        </div>
        <button
          onClick={() => setExpanded(!expanded)}
          className="text-gray-400 hover:text-white text-sm transition-colors"
        >
          {expanded ? 'Collapse' : 'Expand'}
        </button>
      </div>

      {/* Basic Info */}
      <div className="space-y-2 text-sm">
        {showTxHash && (
          <div className="flex items-center gap-2">
            <span className="text-gray-500 min-w-[80px]">Tx Hash:</span>
            <TxHashLink hash={log.tx_hash} />
          </div>
        )}
        {showAddress && (
          <div className="flex items-center gap-2">
            <span className="text-gray-500 min-w-[80px]">Address:</span>
            <AddressLink address={log.address} />
          </div>
        )}
        <div className="flex items-center gap-2">
          <span className="text-gray-500 min-w-[80px]">Block:</span>
          <span className="text-gray-300">{formatNumber(log.block_number)}</span>
        </div>
      </div>

      {/* Decoded Parameters */}
      {decoded?.decoded_params && decoded.decoded_params.length > 0 && (
        <div className="mt-4 pt-4 border-t border-dark-600">
          <h4 className="text-gray-400 text-sm mb-2">Decoded Parameters</h4>
          <div className="space-y-2 bg-dark-700 p-3">
            {decoded.decoded_params.map((param, idx) => (
              <DecodedParamDisplay key={idx} param={param} />
            ))}
          </div>
        </div>
      )}

      {/* Expanded Raw Data */}
      {expanded && (
        <div className="mt-4 pt-4 border-t border-dark-600 space-y-4">
          {/* Topics */}
          <div>
            <h4 className="text-gray-400 text-sm mb-2">Topics</h4>
            <div className="space-y-2 bg-dark-700 p-3">
              <div className="flex items-center gap-2 text-sm">
                <span className="text-gray-500 min-w-[60px]">topic0:</span>
                <div className="flex items-center gap-2">
                  <span className="hash text-gray-300">{truncateHash(log.topic0, 20, 10)}</span>
                  <CopyButton text={log.topic0} />
                </div>
              </div>
              {log.topic1 && (
                <div className="flex items-center gap-2 text-sm">
                  <span className="text-gray-500 min-w-[60px]">topic1:</span>
                  <div className="flex items-center gap-2">
                    <span className="hash text-gray-300">{truncateHash(log.topic1, 20, 10)}</span>
                    <CopyButton text={log.topic1} />
                  </div>
                </div>
              )}
              {log.topic2 && (
                <div className="flex items-center gap-2 text-sm">
                  <span className="text-gray-500 min-w-[60px]">topic2:</span>
                  <div className="flex items-center gap-2">
                    <span className="hash text-gray-300">{truncateHash(log.topic2, 20, 10)}</span>
                    <CopyButton text={log.topic2} />
                  </div>
                </div>
              )}
              {log.topic3 && (
                <div className="flex items-center gap-2 text-sm">
                  <span className="text-gray-500 min-w-[60px]">topic3:</span>
                  <div className="flex items-center gap-2">
                    <span className="hash text-gray-300">{truncateHash(log.topic3, 20, 10)}</span>
                    <CopyButton text={log.topic3} />
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* Data */}
          {log.data && log.data !== '0x' && (
            <div>
              <h4 className="text-gray-400 text-sm mb-2">Data</h4>
              <div className="bg-dark-700 p-3 overflow-x-auto">
                <pre className="hash text-gray-300 text-xs whitespace-pre-wrap break-all">
                  {log.data}
                </pre>
              </div>
            </div>
          )}

          {/* Event Signature */}
          {decoded?.event_signature && (
            <div>
              <h4 className="text-gray-400 text-sm mb-2">Event Signature</h4>
              <div className="bg-dark-700 p-3">
                <code className="text-gray-300 text-sm">{decoded.event_signature}</code>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default function EventLogs({
  logs,
  pagination,
  onPageChange,
  showTxHash = false,
  showAddress = true,
  loading = false,
}: EventLogsProps) {
  const items = logs ?? [];
  if (loading) {
    return (
      <div className="flex items-center justify-center py-8">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-accent-primary"></div>
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <p className="text-gray-400 text-center py-8">No event logs found</p>
    );
  }

  return (
    <div>
      <div className="space-y-4">
        {items.map((log) => (
          <LogCard
            key={`${log.tx_hash}-${log.log_index}`}
            log={log}
            showTxHash={showTxHash}
            showAddress={showAddress}
          />
        ))}
      </div>

      {pagination && pagination.total_pages > 1 && onPageChange && (
        <Pagination
          currentPage={pagination.page}
          totalPages={pagination.total_pages}
          onPageChange={onPageChange}
        />
      )}
    </div>
  );
}
