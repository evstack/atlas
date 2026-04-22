import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Layout } from './components';
import { ThemeProvider } from './context/ThemeContext';
import { BrandingProvider } from './context/BrandingContext';

const BlocksPage = lazy(() => import('./pages/BlocksPage'));
const BlockDetailPage = lazy(() => import('./pages/BlockDetailPage'));
const BlockTransactionsPage = lazy(() => import('./pages/BlockTransactionsPage'));
const TransactionsPage = lazy(() => import('./pages/TransactionsPage'));
const TransactionDetailPage = lazy(() => import('./pages/TransactionDetailPage'));
const AddressPage = lazy(() => import('./pages/AddressPage'));
const AddressesPage = lazy(() => import('./pages/AddressesPage'));
const NFTsPage = lazy(() => import('./pages/NFTsPage'));
const NFTContractPage = lazy(() => import('./pages/NFTContractPage'));
const NFTTokenPage = lazy(() => import('./pages/NFTTokenPage'));
const TokensPage = lazy(() => import('./pages/TokensPage'));
const TokenDetailPage = lazy(() => import('./pages/TokenDetailPage'));
const SearchResultsPage = lazy(() => import('./pages/SearchResultsPage'));
const NotFoundPage = lazy(() => import('./pages/NotFoundPage'));
const WelcomePage = lazy(() => import('./pages/WelcomePage'));
const FaucetPage = lazy(() => import('./pages/FaucetPage'));
const StatusPage = lazy(() => import('./pages/StatusPage'));

function PageLoader() {
  return (
    <div className="card flex h-64 items-center justify-center">
      <span className="kicker">Loading route</span>
    </div>
  );
}

export default function App() {
  return (
    <ThemeProvider>
      <BrandingProvider>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<Layout />}>
            <Route index element={<Suspense fallback={<PageLoader />}><WelcomePage /></Suspense>} />
            <Route path="blocks" element={<Suspense fallback={<PageLoader />}><BlocksPage /></Suspense>} />
            <Route path="blocks/:number" element={<Suspense fallback={<PageLoader />}><BlockDetailPage /></Suspense>} />
            <Route path="blocks/:number/transactions" element={<Suspense fallback={<PageLoader />}><BlockTransactionsPage /></Suspense>} />
            <Route path="transactions" element={<Suspense fallback={<PageLoader />}><TransactionsPage /></Suspense>} />
            <Route path="search" element={<Suspense fallback={<PageLoader />}><SearchResultsPage /></Suspense>} />
            <Route path="addresses" element={<Suspense fallback={<PageLoader />}><AddressesPage /></Suspense>} />
            <Route path="tx/:hash" element={<Suspense fallback={<PageLoader />}><TransactionDetailPage /></Suspense>} />
            <Route path="address/:address" element={<Suspense fallback={<PageLoader />}><AddressPage /></Suspense>} />
            <Route path="nfts" element={<Suspense fallback={<PageLoader />}><NFTsPage /></Suspense>} />
            <Route path="nfts/:contract" element={<Suspense fallback={<PageLoader />}><NFTContractPage /></Suspense>} />
            <Route path="nfts/:contract/:tokenId" element={<Suspense fallback={<PageLoader />}><NFTTokenPage /></Suspense>} />
            <Route path="status" element={<Suspense fallback={<PageLoader />}><StatusPage /></Suspense>} />
            <Route path="tokens" element={<Suspense fallback={<PageLoader />}><TokensPage /></Suspense>} />
            <Route path="tokens/:address" element={<Suspense fallback={<PageLoader />}><TokenDetailPage /></Suspense>} />
            <Route path="faucet" element={<Suspense fallback={<PageLoader />}><FaucetPage /></Suspense>} />
            <Route path="*" element={<Suspense fallback={<PageLoader />}><NotFoundPage /></Suspense>} />
          </Route>
        </Routes>
      </BrowserRouter>
      </BrandingProvider>
    </ThemeProvider>
  );
}
