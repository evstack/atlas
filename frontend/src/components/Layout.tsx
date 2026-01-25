import { Link, NavLink, Outlet } from 'react-router-dom';
import SearchBar from './SearchBar';

export default function Layout() {
  const navLinkClass = ({ isActive }: { isActive: boolean }) =>
    `px-4 py-2 transition-colors duration-150 ${
      isActive
        ? 'text-accent-primary border-b-2 border-accent-primary'
        : 'text-gray-400 hover:text-white'
    }`;

  return (
    <div className="min-h-screen flex flex-col">
      {/* Header */}
      <header className="bg-dark-800 border-b border-dark-600 sticky top-0 z-50">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex items-center justify-between h-16">
            {/* Logo */}
            <Link to="/" className="flex items-center space-x-3">
              <div className="w-8 h-8 bg-accent-primary flex items-center justify-center">
                <span className="text-white font-bold text-lg">E</span>
              </div>
              <span className="text-xl font-bold text-white tracking-tight">Evolve</span>
            </Link>

            {/* Navigation */}
            <nav className="hidden md:flex items-center space-x-2">
              <NavLink to="/blocks" className={navLinkClass}>
                Blocks
              </NavLink>
              <NavLink to="/transactions" className={navLinkClass}>
                Transactions
              </NavLink>
              <NavLink to="/tokens" className={navLinkClass}>
                Tokens
              </NavLink>
              <NavLink to="/nfts" className={navLinkClass}>
                NFTs
              </NavLink>
            </nav>

            {/* Search */}
            <div className="hidden lg:block w-96">
              <SearchBar />
            </div>
          </div>

          {/* Mobile search */}
          <div className="lg:hidden pb-4">
            <SearchBar />
          </div>

          {/* Mobile navigation */}
          <nav className="md:hidden flex items-center space-x-2 pb-4 overflow-x-auto">
            <NavLink to="/blocks" className={navLinkClass}>
              Blocks
            </NavLink>
            <NavLink to="/transactions" className={navLinkClass}>
              Transactions
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

      {/* Main content */}
      <main className="flex-1">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
          <Outlet />
        </div>
      </main>

      {/* Footer */}
      <footer className="bg-dark-800 border-t border-dark-600 py-6">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex flex-col md:flex-row items-center justify-between text-sm text-gray-500">
            <p>Evolve Explorer</p>
            <p className="mt-2 md:mt-0">Powered by Rollkit</p>
          </div>
        </div>
      </footer>
    </div>
  );
}
