import { useState } from 'react';
import type { FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';

export default function SearchBar() {
  const [query, setQuery] = useState('');
  const navigate = useNavigate();

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();

    const trimmedQuery = query.trim();
    if (!trimmedQuery) return;

    // Determine query type and navigate
    if (/^0x[a-fA-F0-9]{64}$/.test(trimmedQuery)) {
      // Transaction hash (66 chars including 0x)
      navigate(`/tx/${trimmedQuery}`);
    } else if (/^0x[a-fA-F0-9]{40}$/.test(trimmedQuery)) {
      // Address (42 chars including 0x)
      navigate(`/address/${trimmedQuery}`);
    } else if (/^\d+$/.test(trimmedQuery)) {
      // Block number
      navigate(`/blocks/${trimmedQuery}`);
    } else {
      // Default to address search
      navigate(`/address/${trimmedQuery}`);
    }

    setQuery('');
  };

  return (
    <form onSubmit={handleSubmit} className="relative">
      <input
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search by Address / Tx Hash / Block Number"
        className="w-full bg-dark-700 border border-dark-500 px-4 py-2 pl-10 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent-primary transition-colors"
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
    </form>
  );
}
