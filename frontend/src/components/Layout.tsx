import { Link, NavLink, Outlet, useLocation } from 'react-router-dom';
import { useEffect, useMemo, useRef, useState } from 'react';
import SearchBar from './SearchBar';
import useLatestBlockHeight from '../hooks/useLatestBlockHeight';
import SmoothCounter from './SmoothCounter';
import logoImg from '../assets/logo.png';
import { BlockStatsContext } from '../context/BlockStatsContext';

export default function Layout() {
  const location = useLocation();
  const isHome = location.pathname === '/';
  const { height, lastUpdatedAt, bps } = useLatestBlockHeight(2000, 1000000);
  const [now, setNow] = useState(() => Date.now());
  const recentlyUpdated = lastUpdatedAt ? (now - lastUpdatedAt) < 10000 : false;
  const [displayedHeight, setDisplayedHeight] = useState<number | null>(null);
  const rafRef = useRef<number | null>(null);
  const displayRafRef = useRef<number | null>(null);
  const lastFrameRef = useRef<number>(0);
  const displayedRef = useRef<number>(0);

  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, []);

  // Smoothly increment displayed height using bps
  useEffect(() => {
    if (height == null) {
      if (displayRafRef.current !== null) {
        cancelAnimationFrame(displayRafRef.current);
      }
      displayRafRef.current = window.requestAnimationFrame(() => {
        setDisplayedHeight(null);
        displayRafRef.current = null;
      });
      return;
    }

    // Initialize displayed to at least current height on first run
    if (displayedHeight == null || height > displayedRef.current) {
      displayedRef.current = Math.max(displayedRef.current || 0, height);
      if (displayRafRef.current !== null) {
        cancelAnimationFrame(displayRafRef.current);
      }
      displayRafRef.current = window.requestAnimationFrame(() => {
        setDisplayedHeight(displayedRef.current);
        displayRafRef.current = null;
      });
    }

    const loop = (t: number) => {
      if (!bps || bps <= 0) {
        // No rate info; just stick to the last known real height
        if (displayedRef.current !== height) {
          displayedRef.current = height;
          setDisplayedHeight(displayedRef.current);
        }
      } else {
        const now = t || performance.now();
        const dt = lastFrameRef.current ? (now - lastFrameRef.current) / 1000 : 0;
        lastFrameRef.current = now;

        // Increase predicted height smoothly by bps * dt
        const predicted = displayedRef.current + bps * dt;
        // Always at least the latest known chain height
        const next = Math.max(height, Math.floor(predicted));
        if (next !== displayedRef.current) {
          displayedRef.current = next;
          setDisplayedHeight(next);
        }
      }
      rafRef.current = window.requestAnimationFrame(loop);
    };

    rafRef.current = window.requestAnimationFrame(loop);
    return () => {
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
      if (displayRafRef.current) cancelAnimationFrame(displayRafRef.current);
      rafRef.current = null;
      displayRafRef.current = null;
      lastFrameRef.current = 0;
    };
  }, [height, bps]);
  const blockTimeLabel = useMemo(() => {
    if (bps !== null && bps > 0) {
      const secs = 1 / bps;
      if (secs < 1) {
        return `${Math.round(secs * 1000)} ms`;
      }
      return `${secs.toFixed(1)} s`;
    }
    return '—';
  }, [bps]);
  const navLinkClass = ({ isActive }: { isActive: boolean }) =>
    `inline-flex items-center h-10 px-4 rounded-full leading-none transition-colors duration-150 ${
      isActive
        ? 'bg-dark-700/70 text-white'
        : 'text-gray-400 hover:text-white hover:bg-dark-700/40'
    }`;

  return (
    <div className="min-h-screen flex flex-col">
      {/* Header */}
      <header className="bg-gradient-to-b from-dark-800 to-dark-900 border-b border-dark-600 sticky top-0 z-50">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="grid grid-cols-3 items-center h-16">
            {/* Logo */}
            <div className="flex md:justify-start justify-center">
              <Link to="/" className="flex items-center" aria-label="Atlas Home">
                <img src={logoImg} alt="Atlas" className="h-12 w-auto rounded-lg" />
              </Link>
            </div>

            {/* Navigation - centered on desktop */}
            <nav className="hidden md:flex items-center justify-center space-x-2 relative z-10">
              <NavLink to="/blocks" className={navLinkClass}>
                Blocks
              </NavLink>
              <NavLink to="/transactions" className={navLinkClass}>
                Transactions
              </NavLink>
              <NavLink to="/addresses" className={navLinkClass}>
                Addresses
              </NavLink>
              <NavLink to="/tokens" className={navLinkClass}>
                Tokens
              </NavLink>
              <NavLink to="/nfts" className={navLinkClass}>
                NFTs
              </NavLink>
            </nav>

            {/* Right status: latest height + live pulse */}
            <div className="hidden md:flex items-center justify-end">
              <div className="flex items-center gap-3 text-sm text-gray-300">
                <span
                  className={`inline-block w-2.5 h-2.5 rounded-full ${recentlyUpdated ? 'bg-red-500 live-dot' : 'bg-gray-600'}`}
                  title={recentlyUpdated ? 'Live updates' : 'Idle'}
                />
                <SmoothCounter value={displayedHeight} />
                <span className="text-gray-600">|</span>
                <span
                  className="font-mono tabular-nums inline-block w-16 text-right whitespace-nowrap"
                >
                  {blockTimeLabel}
                </span>
              </div>
            </div>
          </div>

          {/* Mobile navigation */}
          <nav className="md:hidden flex items-center space-x-2 pb-4 overflow-x-auto">
            <NavLink to="/blocks" className={navLinkClass}>
              Blocks
            </NavLink>
            <NavLink to="/transactions" className={navLinkClass}>
              Transactions
            </NavLink>
            <NavLink to="/addresses" className={navLinkClass}>
              Addresses
            </NavLink>
            <NavLink to="/tokens" className={navLinkClass}>
              Tokens
            </NavLink>
            <NavLink to="/nfts" className={navLinkClass}>
              NFTs
            </NavLink>
          </nav>
        </div>
      </header>

      {/* In-flow search bar under the header (hidden on home hero) */}
      {!isHome && (
        <div className="bg-dark-800/40">
          <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4 flex justify-center">
            <div className="w-full md:w-96">
              <SearchBar />
            </div>
          </div>
        </div>
      )}

      {/* Main content */}
      <main className="flex-1">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
          <BlockStatsContext.Provider value={{ bps, height: displayedHeight }}>
            <Outlet />
          </BlockStatsContext.Provider>
        </div>
      </main>

      {/* Footer */}
      <footer className="bg-dark-800 border-t border-dark-600 py-6">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex flex-col md:flex-row items-center justify-between text-sm text-gray-500">
            <p></p>
          </div>
        </div>
      </footer>
    </div>
  );
}
