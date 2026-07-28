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
import { LibrariesAdminPage } from './routes/LibrariesAdminPage';
import { SettingsPage } from './routes/SettingsPage';
import { StatsPage } from './routes/StatsPage';
import { PlayerRoot } from './components/player/PlayerRoot';
import { InstallBanner } from './components/ui/InstallBanner';
import { NetworkBanner } from './components/ui/NetworkBanner';
import { Toaster } from './components/ui/Toaster';
import { startHealthPolling } from './lib/healthPoll';
import { startProgressSyncOnReconnect } from './lib/playerResume';
import { startTokenRefresh } from './lib/authRefresh';

const queryClient = new QueryClient();
startHealthPolling();
startProgressSyncOnReconnect();
startTokenRefresh();

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
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/stats" element={<StatsPage />} />
            <Route element={<RequireOrganize />}>
              <Route path="/organize" element={<OrganizePage />} />
            </Route>
            <Route element={<RequireAdmin />}>
              <Route path="/admin/users" element={<UserManagementPage />} />
              <Route path="/admin/libraries" element={<LibrariesAdminPage />} />
            </Route>
          </Route>
        </Routes>
        <PlayerRoot />
        <InstallBanner />
        <NetworkBanner />
        <Toaster />
      </BrowserRouter>
    </QueryClientProvider>
  );
}
