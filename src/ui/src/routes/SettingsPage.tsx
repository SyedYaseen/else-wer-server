import { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useAuthStore } from '../store/auth';
import { useThemeStore, type ThemeMode } from '../store/theme';
import { Button } from '../components/ui/Button';
import { Card } from '../components/ui/Card';
import { BottomSheet } from '../components/ui/BottomSheet';
import { Tabs } from '../components/ui/Tabs';
import { ChevronRightIcon } from '../components/organize/icons';
import { TrashIcon } from '../components/ui/icons';
import { listDownloadedBooks, deleteBookDownload, clearAllDownloads } from '../offline/storage';
import { showToast } from '../lib/toast';
import { formatBytes } from '../lib/format';
import './settings.css';

interface DownloadedBook {
  bookId: number;
  title: string;
  author: string;
  totalBytes: number;
}

type SheetState = { kind: 'delete-one'; bookId: number; title: string } | { kind: 'clear-all' } | null;

export function SettingsPage() {
  const isAdmin = useAuthStore((s) => s.isAdmin());
  const logout = useAuthStore((s) => s.logout);
  const themeMode = useThemeStore((s) => s.mode);
  const setThemeMode = useThemeStore((s) => s.setMode);
  const navigate = useNavigate();
  const [downloads, setDownloads] = useState<DownloadedBook[] | null>(null);
  const [sheet, setSheet] = useState<SheetState>(null);

  async function loadDownloads() {
    const rows = await listDownloadedBooks();
    setDownloads(
      rows.map((r) => ({
        bookId: r.bookId,
        title: r.book.title,
        author: r.book.author,
        totalBytes: r.files.reduce((sum, f) => sum + (f.file_size ?? 0), 0),
      })),
    );
  }

  useEffect(() => {
    void loadDownloads();
  }, []);

  async function handleDeleteOne(bookId: number, title: string) {
    try {
      await deleteBookDownload(bookId);
      await loadDownloads();
      setSheet(null);
      showToast(`Removed "${title}" from downloads`, 'success');
    } catch (err) {
      showToast(`Couldn't remove download: ${err instanceof Error ? err.message : String(err)}`, 'error');
    }
  }

  async function handleClearAll() {
    try {
      await clearAllDownloads();
      await loadDownloads();
      setSheet(null);
      showToast('Cleared all downloads', 'success');
    } catch (err) {
      showToast(`Couldn't clear downloads: ${err instanceof Error ? err.message : String(err)}`, 'error');
    }
  }

  return (
    <div className="settings-page">
      <div className="settings-header">
        <div>
          <h1 className="settings-title">Settings</h1>
          <p className="settings-subtitle">Account, downloads, and app preferences</p>
        </div>
        <Link className="settings-back-link" to="/">
          <ChevronRightIcon size={16} className="rotate-180" /> Back to library
        </Link>
      </div>

      <h2 className="settings-section-title">Account</h2>
      <Card>
        {isAdmin && (
          <button className="settings-account-row" onClick={() => navigate('/admin/users')}>
            Manage users
          </button>
        )}
        <button className="settings-account-row" onClick={logout}>
          Log out
        </button>
      </Card>

      <h2 className="settings-section-title">Stats</h2>
      <Card>
        <button className="settings-account-row" onClick={() => navigate('/stats')}>
          Listening stats
        </button>
      </Card>

      <h2 className="settings-section-title">Downloads</h2>
      {downloads === null && <div className="settings-state">Loading downloads…</div>}
      {downloads?.length === 0 && <div className="settings-state">No downloaded books</div>}
      {downloads && downloads.length > 0 && (
        <>
          <div className="settings-downloads-list">
            {downloads.map((d) => (
              <Card key={d.bookId} className="settings-download-row">
                <div className="settings-download-info">
                  <span className="settings-download-title">{d.title}</span>
                  <span className="settings-download-meta">
                    {d.author} · {formatBytes(d.totalBytes)}
                  </span>
                </div>
                <Button
                  variant="ghost"
                  aria-label="Remove download"
                  onClick={() => setSheet({ kind: 'delete-one', bookId: d.bookId, title: d.title })}
                >
                  <TrashIcon size={16} />
                </Button>
              </Card>
            ))}
          </div>
          <Button variant="destructive" onClick={() => setSheet({ kind: 'clear-all' })}>
            Clear all downloads
          </Button>
        </>
      )}

      <h2 className="settings-section-title">Appearance</h2>
      <Tabs
        tabs={[
          { id: 'system', label: 'System' },
          { id: 'light', label: 'Light' },
          { id: 'dark', label: 'Dark' },
        ]}
        activeId={themeMode}
        onChange={(id) => setThemeMode(id as ThemeMode)}
      />

      <BottomSheet open={sheet?.kind === 'delete-one'} onClose={() => setSheet(null)}>
        <h3 className="sheet-title">Remove download</h3>
        {sheet?.kind === 'delete-one' && (
          <>
            <p className="settings-confirm-text">
              Remove <strong>{sheet.title}</strong> from this device? You can download it again anytime.
            </p>
            <div className="settings-confirm-actions">
              <Button variant="ghost" onClick={() => setSheet(null)}>
                Cancel
              </Button>
              <Button variant="destructive" onClick={() => handleDeleteOne(sheet.bookId, sheet.title)}>
                Remove
              </Button>
            </div>
          </>
        )}
      </BottomSheet>

      <BottomSheet open={sheet?.kind === 'clear-all'} onClose={() => setSheet(null)}>
        <h3 className="sheet-title">Clear all downloads</h3>
        <p className="settings-confirm-text">
          Remove all {downloads?.length ?? 0} downloaded books from this device?
        </p>
        <div className="settings-confirm-actions">
          <Button variant="ghost" onClick={() => setSheet(null)}>
            Cancel
          </Button>
          <Button variant="destructive" onClick={handleClearAll}>
            Clear all
          </Button>
        </div>
      </BottomSheet>
    </div>
  );
}
