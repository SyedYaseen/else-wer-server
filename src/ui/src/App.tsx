import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RequireAuth } from './routes/RequireAuth';
import { RequireAdmin } from './routes/RequireAdmin';
import { RequireOrganize } from './routes/RequireOrganize';
import { LoginPage } from './routes/LoginPage';
import { LibraryPage } from './routes/LibraryPage';
import { BookDetailPage } from './routes/BookDetailPage';
import { PlayerPage } from './routes/PlayerPage';
import { OrganizePage } from './routes/OrganizePage';
import { UserManagementPage } from './routes/UserManagementPage';
import { PlayerRoot } from './components/player/PlayerRoot';
import { InstallBanner } from './components/ui/InstallBanner';

const queryClient = new QueryClient();

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route element={<RequireAuth />}>
            <Route path="/" element={<LibraryPage />} />
            <Route path="/book/:id" element={<BookDetailPage />} />
            <Route path="/player/:id" element={<PlayerPage />} />
            <Route element={<RequireOrganize />}>
              <Route path="/organize" element={<OrganizePage />} />
            </Route>
            <Route element={<RequireAdmin />}>
              <Route path="/admin/users" element={<UserManagementPage />} />
            </Route>
          </Route>
        </Routes>
        <PlayerRoot />
        <InstallBanner />
      </BrowserRouter>
    </QueryClientProvider>
  );
}
