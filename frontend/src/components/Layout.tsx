import { useMemo } from 'react';
import { Link, NavLink, Outlet, useLocation } from 'react-router-dom';
import SearchBar from './SearchBar';
import useBlockSSE from '../hooks/useBlockSSE';
import SmoothCounter from './SmoothCounter';
import { BlockStatsContext } from '../context/BlockStatsContext';
import { useTheme } from '../hooks/useTheme';
import { useBranding } from '../hooks/useBranding';
import { getDefaultLogo } from '../assets/defaultLogos';

const NAV_ITEMS = [
  { to: '/blocks', label: 'Blocks' },
  { to: '/transactions', label: 'Transactions' },
  { to: '/addresses', label: 'Addresses' },
  { to: '/tokens', label: 'Tokens' },
  { to: '/nfts', label: 'NFTs' },
  { to: '/status', label: 'Status' },
];

export default function Layout() {
  const location = useLocation();
  const isHome = location.pathname === '/';
  const sse = useBlockSSE();

  const blockTimeLabel = useMemo(() => {
    if (sse.bps !== null && sse.bps > 0) {
      const secs = 1 / sse.bps;
      if (secs < 1) return `${Math.round(secs * 1000)} ms`;
      return `${secs.toFixed(1)} s`;
    }
    return '—';
  }, [sse.bps]);

  const navLinkClass = ({ isActive }: { isActive: boolean }) =>
    `inline-flex items-center h-10 px-4 rounded-full leading-none text-[12px] uppercase tracking-[0.16em] font-brandmono transition-colors duration-150 ${
      isActive
        ? 'bg-dark-700 text-fg border border-border/80'
        : 'text-gray-500 hover:text-fg hover:bg-dark-700/70 border border-transparent'
    }`;

  const { theme, toggleTheme } = useTheme();
  const isDark = theme === 'dark';
  const { chainName, logoUrl, faucet } = useBranding();
  const logoSrc = logoUrl || getDefaultLogo(theme);

  return (
    <div className="app-shell">
      <header className="app-content sticky top-0 z-50 px-4 pt-4 md:px-6 md:pt-6">
        <div className="mx-auto max-w-[92rem]">
          <div className="rounded-[999px] border border-border/80 bg-surface-800/90 px-3 py-3 shadow-[0_24px_64px_rgba(0,0,0,0.06)] backdrop-blur">
            <div className="flex items-center justify-between gap-3">
              <Link
                to="/"
                className="flex items-center gap-3 pl-2"
                aria-label={`${chainName} Home`}
              >
                <img
                  src={logoSrc}
                  alt={chainName}
                  className="h-11 w-auto"
                />
                <div className="hidden lg:block">
                  <p className="kicker">Explorer</p>
                  <p className="text-base font-medium tracking-[-0.03em] text-fg">{chainName}</p>
                </div>
              </Link>

              <nav className="hidden xl:flex items-center justify-center gap-2">
                {NAV_ITEMS.map((item) => (
                  <NavLink key={item.to} to={item.to} className={navLinkClass}>
                    {item.label}
                  </NavLink>
                ))}
                {faucet.enabled && (
                  <NavLink to="/faucet" className={navLinkClass}>
                    Faucet
                  </NavLink>
                )}
              </nav>

              <div className="flex items-center gap-2">
                <div className="hidden md:flex items-center gap-3 rounded-full border border-border/80 bg-surface-700/85 px-4 py-2 text-xs">
                  <span
                    className={`inline-block h-2.5 w-2.5 rounded-full ${
                      sse.connected
                        ? 'bg-green-500 live-dot'
                        : sse.height !== null
                          ? 'bg-brand-lavender live-dot'
                          : 'bg-gray-600'
                    }`}
                    title={
                      sse.connected
                        ? 'SSE connected'
                        : sse.height !== null
                          ? 'Polling'
                          : 'Idle'
                    }
                  />
                  <span className="kicker !text-[10px] !tracking-[0.16em]">Live</span>
                  <span className="text-sm font-medium tracking-[-0.02em] text-fg">
                    <SmoothCounter value={sse.height} />
                  </span>
                  <span className="text-fg-faint">•</span>
                  <span className="font-mono tabular-nums text-fg-subtle whitespace-nowrap">
                    {blockTimeLabel}
                  </span>
                </div>
                <button
                  type="button"
                  onClick={toggleTheme}
                  aria-label={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
                  className="group inline-flex items-center justify-center h-11 w-11 rounded-full border border-border/80 bg-surface-700/85 hover:bg-dark-700/85 hover:-translate-y-px hover:shadow-md transition-all duration-150"
                >
                  {isDark ? (
                    <svg
                      className="h-5 w-5 text-gray-200"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      aria-hidden="true"
                    >
                      <path d="M21 14.5a8.5 8.5 0 01-11.5-11.5 8.5 8.5 0 1011.5 11.5z" />
                    </svg>
                  ) : (
                    <svg
                      className="h-5 w-5 text-gray-400 group-hover:text-fg transition-colors duration-150"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      aria-hidden="true"
                    >
                      <circle cx="12" cy="12" r="4" />
                      <path d="M12 2v2m0 16v2M20 12h2M2 12h2M17.657 6.343l-1.414 1.414M7.757 16.243l-1.414 1.414M6.343 6.343l1.414 1.414M16.243 16.243l1.414 1.414" />
                    </svg>
                  )}
                </button>
              </div>
            </div>

            <nav className="mt-3 flex xl:hidden items-center gap-2 overflow-x-auto pb-1">
              {NAV_ITEMS.map((item) => (
                <NavLink key={item.to} to={item.to} className={navLinkClass}>
                  {item.label}
                </NavLink>
              ))}
              {faucet.enabled && (
                <NavLink to="/faucet" className={navLinkClass}>
                  Faucet
                </NavLink>
              )}
            </nav>
          </div>
        </div>
      </header>

      {!isHome && (
        <div className="app-content px-4 pt-5 md:px-6">
          <div className="mx-auto max-w-[92rem]">
            <div className="mx-auto w-full max-w-[560px] shell-search">
              <SearchBar variant="compact" />
            </div>
          </div>
        </div>
      )}

      <main className="app-content flex-1 px-4 pb-16 pt-2 md:px-6 md:pt-4">
        <div className="mx-auto max-w-[92rem]">
          <BlockStatsContext.Provider
            value={{
              bps: sse.bps,
              height: sse.height,
              latestBlockEvent: sse.latestBlock,
              sseConnected: sse.connected,
              subscribeDa: sse.subscribeDa,
              subscribeDaResync: sse.subscribeDaResync,
            }}
          >
            <Outlet />
          </BlockStatsContext.Provider>
        </div>
      </main>

      <footer className="app-content px-4 pb-6 md:px-6">
        <div className="mx-auto max-w-[92rem]">
          <div className="rounded-[2rem] border border-border/70 bg-surface-800/80 px-6 py-5 text-sm text-fg-subtle backdrop-blur">
            <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
              <p>{chainName} Atlas explorer</p>
              <p className="font-brandmono text-[11px] uppercase tracking-[0.18em]">Live chain surface</p>
            </div>
          </div>
        </div>
      </footer>
    </div>
  );
}
