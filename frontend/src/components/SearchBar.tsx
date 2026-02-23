import { useEffect, useRef, useState } from 'react';
import type { FormEvent, KeyboardEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { search as apiSearch } from '../api/search';
import type { AnySearchResult, BlockSearchResult, TransactionSearchResult } from '../types';

function isBlockResult(result: AnySearchResult): result is BlockSearchResult {
  return result.type === 'block';
}

function isTransactionResult(result: AnySearchResult): result is TransactionSearchResult {
  return result.type === 'transaction';
}

function getPrimaryText(result: AnySearchResult): string {
  switch (result.type) {
    case 'block':
      return `Block #${result.number}`;
    case 'transaction':
      return `Tx ${result.hash}`;
    case 'address':
      return `Address ${result.address}`;
    case 'nft':
      return `NFT ${result.contract_address} #${result.token_id}`;
    case 'nft_collection':
      return `NFT Collection ${result.name || ''}`;
  }
}

function getSecondaryText(result: AnySearchResult): string {
  switch (result.type) {
    case 'block':
      return `Hash ${result.hash}`;
    case 'transaction':
      return `Block ${result.block_number}`;
    case 'address':
      return (result.is_contract || ('address_type' in result && result.address_type === 'contract') ? 'Contract' : 'EOA');
    case 'nft':
      return result.name || 'NFT';
    case 'nft_collection':
      return result.address;
  }
}

export default function SearchBar() {
  const [query, setQuery] = useState('');
  const navigate = useNavigate();
  const [suggestions, setSuggestions] = useState<AnySearchResult[]>([]);
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [highlight, setHighlight] = useState<number>(-1);
  const abortRef = useRef<AbortController | null>(null);
  const debounceRef = useRef<number | null>(null);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();

    const trimmedQuery = query.trim();
    if (!trimmedQuery) return;

    // If a suggestion is highlighted, go to it
    if (open && highlight >= 0) {
      if (highlight < suggestions.length) {
        navigateToResult(suggestions[highlight]);
        return;
      }
      if (highlight === suggestions.length) {
        navigate(`/search?q=${encodeURIComponent(trimmedQuery)}`);
        setOpen(false);
        setSuggestions([]);
        setHighlight(-1);
        setQuery('');
        return;
      }
    }

    // Determine query type and navigate (fallback behavior)
    if (/^0x[a-fA-F0-9]{64}$/.test(trimmedQuery)) {
      // 32-byte hash: could be block or transaction. Prefer block if present.
      try {
        const res = await apiSearch(trimmedQuery);
        const block = res.results.find(isBlockResult);
        if (block) { navigate(`/blocks/${block.number}`); return; }
        const tx = res.results.find(isTransactionResult);
        if (tx) { navigate(`/tx/${tx.hash}`); return; }
        navigate(`/search?q=${encodeURIComponent(trimmedQuery)}`);
      } catch {
        navigate(`/search?q=${encodeURIComponent(trimmedQuery)}`);
      }
    } else if (/^0x[a-fA-F0-9]{40}$/.test(trimmedQuery)) {
      // Address (42 chars including 0x)
      navigate(`/address/${trimmedQuery}`);
    } else if (/^[\d, _]+$/.test(trimmedQuery)) {
      // Block number (allow common separators like commas, spaces, underscores)
      const numericOnly = trimmedQuery.replace(/\D/g, '');
      if (numericOnly.length > 0) {
        navigate(`/blocks/${numericOnly}`);
      }
    } else {
      // Fallback: open full search results
      navigate(`/search?q=${encodeURIComponent(trimmedQuery)}`);
    }

    setQuery('');
    setSuggestions([]);
    setOpen(false);
    setHighlight(-1);
  };

  const navigateToResult = (r: AnySearchResult) => {
    switch (r.type) {
      case 'block':
        navigate(`/blocks/${r.number}`);
        break;
      case 'transaction':
        navigate(`/tx/${r.hash}`);
        break;
      case 'address':
        navigate(`/address/${r.address}`);
        break;
      case 'nft':
        navigate(`/nfts/${r.contract_address}/${r.token_id}`);
        break;
      case 'nft_collection':
        navigate(`/nfts/${r.address}`);
        break;
      default:
        break;
    }
    setOpen(false);
    setSuggestions([]);
    setHighlight(-1);
    setQuery('');
  };

  // Debounced live search
  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setSuggestions([]);
      setOpen(false);
      setHighlight(-1);
      if (abortRef.current) abortRef.current.abort();
      return;
    }

    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    // Open suggestions immediately with a tiny spinner
    setOpen(true);
    setLoading(true);
    setSuggestions([]);
    debounceRef.current = window.setTimeout(async () => {
      try {
        if (abortRef.current) abortRef.current.abort();
        const controller = new AbortController();
        abortRef.current = controller;
        const res = await apiSearch(q);
        setSuggestions(res.results || []);
        setHighlight(-1);
      } catch {
        // ignore errors for suggestions
      } finally {
        setLoading(false);
      }
    }, 200);

    return () => {
      if (debounceRef.current) window.clearTimeout(debounceRef.current);
    };
  }, [query]);

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (!open) return;
    const totalItems = suggestions.length + 1; // +1 for View All
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlight((i) => (i + 1) % totalItems);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlight((i) => (i - 1 + totalItems) % totalItems);
    } else if (e.key === 'Escape') {
      setOpen(false);
      setHighlight(-1);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="relative">
      <input
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={onKeyDown}
        placeholder="Search Address / Tx Hash / Block / Token / NFT"
        className="w-full bg-dark-700/80 backdrop-blur border border-dark-500 px-4 py-2 pl-10 text-sm text-fg placeholder-gray-500 rounded-full shadow-md shadow-black/20 focus:outline-none focus:border-accent-primary focus:ring-2 focus:ring-accent-primary/40 transition"
      />
      <svg
        className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
        />
      </svg>

      {open && (
        <div className="absolute left-0 right-0 mt-2 bg-dark-800/95 border border-dark-600 rounded-xl shadow-xl overflow-hidden z-40">
          {loading && suggestions.length === 0 ? (
            <div className="px-3 py-3 text-sm text-gray-400 flex items-center gap-2">
              <svg className="w-4 h-4 animate-spin text-gray-400" viewBox="0 0 24 24">
                <circle className="opacity-20" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" fill="none" />
                <path className="opacity-80" fill="currentColor" d="M4 12a8 8 0 018-8v4a4 4 0 00-4 4H4z" />
              </svg>
              Searching…
            </div>
          ) : (
            <ul role="listbox" className="max-h-72 overflow-y-auto">
              {suggestions.length === 0 && (
                <li className="px-3 py-2 text-sm text-gray-400 select-none cursor-default">No results found</li>
              )}
              {suggestions.map((r, idx) => (
                <li
                  key={`${r.type}-${idx}`}
                  role="option"
                  aria-selected={highlight === idx}
                  className={`px-3 py-2 cursor-pointer flex items-center gap-3 ${highlight === idx ? 'bg-dark-700/70' : 'hover:bg-dark-700/40'}`}
                  onMouseEnter={() => setHighlight(idx)}
                  onMouseDown={(e) => { e.preventDefault(); }}
                  onClick={() => navigateToResult(r)}
                >
                  <TypeIcon type={r.type} />
                  <div className="min-w-0">
                    <div className="text-sm text-fg truncate">
                      {getPrimaryText(r)}
                    </div>
                    <div className="text-xs text-gray-500 truncate">
                      {getSecondaryText(r)}
                    </div>
                  </div>
                </li>
              ))}
              {/* View all results */}
              <li
                role="option"
                aria-selected={highlight === suggestions.length}
                className={`px-3 py-2 cursor-pointer flex items-center gap-3 border-t border-dark-700 ${highlight === suggestions.length ? 'bg-dark-700/70' : 'hover:bg-dark-700/40'}`}
                onMouseEnter={() => setHighlight(suggestions.length)}
                onMouseDown={(e) => { e.preventDefault(); }}
                onClick={() => {
                  navigate(`/search?q=${encodeURIComponent(query.trim())}`);
                  setOpen(false);
                  setSuggestions([]);
                  setHighlight(-1);
                }}
              >
                <svg className="w-5 h-5 text-gray-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
                <div className="min-w-0">
                  <div className="text-sm text-fg truncate">View all results for “{query.trim()}”</div>
                </div>
              </li>
            </ul>
          )}
        </div>
      )}
    </form>
  );
}

function TypeIcon({ type }: { type: AnySearchResult['type'] }) {
  const cls = 'w-5 h-5 text-gray-300';
  if (type === 'block') {
    return (
      <svg className={cls} fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7l9-4 9 4-9 4-9-4z" />
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 17l9 4 9-4" />
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 12l9 4 9-4" />
      </svg>
    );
  }
  if (type === 'transaction') {
    return (
      <svg className={cls} fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 9l4 4m0 0l-4 4m4-4H7" />
      </svg>
    );
  }
  if (type === 'address') {
    return (
      <svg className={cls} fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 11c1.656 0 3-1.567 3-3.5S13.656 4 12 4 9 5.567 9 7.5 10.344 11 12 11z" />
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19.5 20a7.5 7.5 0 10-15 0" />
      </svg>
    );
  }
  // nft or nft_collection
  return (
    <svg className={cls} fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 2l7 4v6c0 5-3.5 9-7 10-3.5-1-7-5-7-10V6l7-4z" />
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4" />
    </svg>
  );
}
