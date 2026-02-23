import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Layout } from './components';
import {
  BlocksPage,
  BlockDetailPage,
  BlockTransactionsPage,
  TransactionsPage,
  TransactionDetailPage,
  AddressPage,
  NFTsPage,
  NFTContractPage,
  NFTTokenPage,
  TokensPage,
  TokenDetailPage,
  NotFoundPage,
  WelcomePage,
  SearchResultsPage,
  AddressesPage,
} from './pages';
import { ThemeProvider } from './context/ThemeContext';

export default function App() {
  return (
    <ThemeProvider>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<Layout />}>
            <Route index element={<WelcomePage />} />
            <Route path="blocks" element={<BlocksPage />} />
            <Route path="blocks/:number" element={<BlockDetailPage />} />
            <Route path="blocks/:number/transactions" element={<BlockTransactionsPage />} />
            <Route path="transactions" element={<TransactionsPage />} />
            <Route path="search" element={<SearchResultsPage />} />
            <Route path="addresses" element={<AddressesPage />} />
            <Route path="tx/:hash" element={<TransactionDetailPage />} />
            <Route path="address/:address" element={<AddressPage />} />
            <Route path="nfts" element={<NFTsPage />} />
            <Route path="nfts/:contract" element={<NFTContractPage />} />
            <Route path="nfts/:contract/:tokenId" element={<NFTTokenPage />} />
            <Route path="tokens" element={<TokensPage />} />
            <Route path="tokens/:address" element={<TokenDetailPage />} />
            <Route path="*" element={<NotFoundPage />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </ThemeProvider>
  );
}
