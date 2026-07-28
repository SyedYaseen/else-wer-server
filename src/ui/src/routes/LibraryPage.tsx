import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { useAuthStore } from '../store/auth';
import { Button } from '../components/ui/Button';
import { ActionMenu } from '../components/ui/ActionMenu';
import { InstallSheet } from '../components/ui/InstallSheet';
import { Tabs } from '../components/ui/Tabs';
import { BookCard } from '../components/library/BookCard';
import { BookGroupSection } from '../components/library/BookGroupSection';
import { SetSeriesSheet } from '../components/library/SetSeriesSheet';
import { SearchBar } from '../components/library/SearchBar';
import { ContinueListeningRow } from '../components/library/ContinueListeningRow';
import { useLibraryBooks, useLibraries, useInProgressBooks, useRescan } from '../hooks/useLibraryBooks';
import { useBookSearch } from '../hooks/useBookSearch';
import { useInstallPrompt } from '../pwa/useInstallPrompt';
import { buildContinueListening, type ContinueListeningItem } from '../lib/continueListening';
import { listDownloadedBooks } from '../offline/storage';
import { useNetworkStatusStore } from '../store/networkStatus';
import { getLocalProgress } from '../lib/playerResume';
import { groupByAuthor, groupBySeries } from '../lib/groupBooks';
import { RefreshIcon, ChecklistIcon, DownloadIcon, SettingsIcon } from '../components/ui/icons';
import { showToast } from '../lib/toast';
import '../components/library/library.css';

const LIBRARY_TABS = [
  { id: 'all', label: 'All' },
  { id: 'author', label: 'Author' },
  { id: 'series', label: 'Series' },
];

export function LibraryPage() {
  const username = useAuthStore((s) => s.claims?.username);
  const canOrganize = useAuthStore((s) => s.canOrganize());
  const navigate = useNavigate();
  const [scanning, setScanning] = useState(false);
  const { canInstall, isIOS, promptInstall } = useInstallPrompt();
  const [installSheetOpen, setInstallSheetOpen] = useState(false);

  function handleInstallClick() {
    if (isIOS) {
      setInstallSheetOpen(true);
    } else {
      void promptInstall();
    }
  }

  const { data: books = [], isLoading, isError, refetch } = useLibraryBooks();
  const { data: libraries = [] } = useLibraries();
  const { data: progress = [] } = useInProgressBooks();

  // Offline shelf: when the live library can't load, fall back to downloaded
  // books from IndexedDB so a cold start with the server unreachable still
  // reaches playback instead of dead-ending on the error state. Keyed on
  // !reachable as well as isError: the health poll flags an unreachable server
  // within seconds, while the query's retries+timeouts can take 30s+ to reach
  // isError on a cold start (fetches hanging on dead wifi).
  const reachable = useNetworkStatusStore((s) => s.reachable);
  const serverDown = isError || !reachable;
  const { data: offlineBooks = [] } = useQuery({
    queryKey: ['offline-library'],
    queryFn: listDownloadedBooks,
    enabled: serverDown,
  });
  const offlineMode = serverDown && books.length === 0 && offlineBooks.length > 0;

  // Locally cached progress only (one row per book) — the per-file server rows
  // buildContinueListening sums are unreachable here, so progressMs is the
  // current file's position: an underestimate, fine for the offline bar.
  const offlineContinue: ContinueListeningItem[] = offlineMode
    ? offlineBooks
        .flatMap(({ book }) => {
          const [local] = getLocalProgress(book.id);
          return local && !local.complete
            ? [{ book, progressMs: local.progress_ms, updatedAt: local.updated_at }]
            : [];
        })
        .sort((a, b) => (a.updatedAt < b.updatedAt ? 1 : -1))
    : [];
  const rescan = useRescan();
  const [activeTab, setActiveTab] = useState('all');
  const [libraryFilter, setLibraryFilter] = useState<string>('all');

  const scopedBooks =
    libraryFilter === 'all' ? books : books.filter((b) => String(b.library_id) === libraryFilter);
  const libraryTabs = [
    { id: 'all', label: 'All Libraries' },
    ...libraries.map((l) => ({ id: String(l.id), label: l.name })),
  ];

  const [selectMode, setSelectMode] = useState(false);
  const [selectedBookIds, setSelectedBookIds] = useState<Set<number>>(new Set());
  const [seriesSheetOpen, setSeriesSheetOpen] = useState(false);

  function toggleSelect(bookId: number) {
    setSelectedBookIds((prev) => {
      const next = new Set(prev);
      if (next.has(bookId)) next.delete(bookId);
      else next.add(bookId);
      return next;
    });
  }

  function exitSelectMode() {
    setSelectMode(false);
    setSelectedBookIds(new Set());
  }

  const selectedBooks = books
    .filter((b) => selectedBookIds.has(b.id))
    .map((b) => ({ id: b.id, title: b.title, sequence: b.series_sequence }));

  const allSearch = useBookSearch(scopedBooks);
  const authorSearch = useBookSearch(scopedBooks);
  const seriesSearch = useBookSearch(scopedBooks);

  const authorGroups = groupByAuthor(authorSearch.filtered);
  const seriesGroups = groupBySeries(seriesSearch.filtered);

  async function handleRescan() {
    setScanning(true);
    try {
      await rescan();
    } catch {
      showToast("Couldn't scan library", 'error');
    } finally {
      setScanning(false);
    }
  }

  const continueListening = allSearch.hasQuery ? [] : buildContinueListening(scopedBooks, progress);

  return (
    <div className="library-page">
      <div className="library-header">
        <div>
          <h1 className="library-title">Library</h1>
          <p className="library-subtitle">Signed in as {username}</p>
          {books.length > 0 && (
            <p className="library-count">
              {books.length} {books.length === 1 ? 'audiobook' : 'audiobooks'}
            </p>
          )}
          {offlineMode && <p className="library-count">Offline — showing downloaded books</p>}
        </div>
        {!offlineMode && (
        <div className="library-actions">
          <Button variant="secondary" onClick={handleRescan} disabled={scanning}>
            <RefreshIcon size={16} className={scanning ? 'spin' : undefined} /> {scanning ? 'Scanning…' : 'Rescan'}
          </Button>
          {canOrganize && (
            <Link className="library-organize-link" to="/organize">
              Organize
            </Link>
          )}
          <ActionMenu
            items={[
              books.length > 0 &&
                !selectMode && {
                  label: 'Select books',
                  icon: <ChecklistIcon size={16} />,
                  onClick: () => setSelectMode(true),
                },
              canInstall && {
                label: 'Install app',
                icon: <DownloadIcon size={16} />,
                onClick: handleInstallClick,
              },
              { label: 'Settings', icon: <SettingsIcon size={16} />, onClick: () => navigate('/settings') },
            ]}
          />
        </div>
        )}
      </div>
      <InstallSheet open={installSheetOpen} onClose={() => setInstallSheetOpen(false)} />

      {isLoading && !books.length && !offlineMode && (
        <div className="library-state">Loading library…</div>
      )}

      {serverDown && !books.length && !offlineMode && !isLoading && (
        <div className="library-state">
          <div className="library-state-title">Couldn't load library</div>
          <p>Check your server connection and try again.</p>
          <Button variant="primary" onClick={() => refetch()}>
            Try Again
          </Button>
        </div>
      )}

      {offlineMode && (
        <>
          <ContinueListeningRow items={offlineContinue} />
          <div className="library-grid">
            {offlineBooks.map(({ book }) => (
              <BookCard key={book.id} book={book} />
            ))}
          </div>
        </>
      )}

      {!isLoading && !serverDown && books.length === 0 && (
        <div className="library-state">
          <div className="library-state-title">No audiobooks yet</div>
          <p>Scan your server to discover audiobooks.</p>
          <Button variant="primary" onClick={handleRescan} disabled={scanning}>
            <RefreshIcon size={16} className={scanning ? 'spin' : undefined} /> {scanning ? 'Scanning…' : 'Scan for audiobooks'}
          </Button>
        </div>
      )}

      {books.length > 0 && (
        <>
          <ContinueListeningRow items={continueListening} />
          {libraries.length > 1 && (
            <Tabs tabs={libraryTabs} activeId={libraryFilter} onChange={setLibraryFilter} />
          )}
          <Tabs tabs={LIBRARY_TABS} activeId={activeTab} onChange={setActiveTab} />

          {activeTab === 'all' && (
            <>
              <SearchBar
                value={allSearch.query}
                onChange={allSearch.setQuery}
                resultCount={allSearch.filtered.length}
                hasQuery={allSearch.hasQuery}
              />
              {allSearch.hasQuery && allSearch.filtered.length === 0 ? (
                <div className="library-state">
                  <div className="library-state-title">No results</div>
                  <p>Try searching by title, author, or series</p>
                </div>
              ) : (
                <div className="library-grid">
                  {allSearch.filtered.map((book) => (
                    <BookCard
                      key={book.id}
                      book={book}
                      selectable={selectMode}
                      selected={selectedBookIds.has(book.id)}
                      onToggleSelect={toggleSelect}
                    />
                  ))}
                </div>
              )}
            </>
          )}

          {activeTab === 'author' && (
            <>
              <SearchBar
                value={authorSearch.query}
                onChange={authorSearch.setQuery}
                resultCount={authorSearch.filtered.length}
                hasQuery={authorSearch.hasQuery}
              />
              {authorGroups.length === 0 ? (
                <div className="library-state">
                  <div className="library-state-title">No results</div>
                  <p>Try searching by title, author, or series</p>
                </div>
              ) : (
                authorGroups.map((group) => (
                  <BookGroupSection
                    key={group.key}
                    label={group.label}
                    books={group.books}
                    selectable={selectMode}
                    selectedIds={selectedBookIds}
                    onToggleSelect={toggleSelect}
                  />
                ))
              )}
            </>
          )}

          {activeTab === 'series' && (
            <>
              <SearchBar
                value={seriesSearch.query}
                onChange={seriesSearch.setQuery}
                resultCount={seriesSearch.filtered.length}
                hasQuery={seriesSearch.hasQuery}
              />
              {seriesGroups.length === 0 ? (
                <div className="library-state">
                  <div className="library-state-title">No results</div>
                  <p>
                    {seriesSearch.hasQuery
                      ? 'Try searching by title, author, or series'
                      : 'No books are part of a series yet'}
                  </p>
                </div>
              ) : (
                seriesGroups.map((group) => (
                  <BookGroupSection
                    key={group.key}
                    label={group.label}
                    books={group.books}
                    selectable={selectMode}
                    selectedIds={selectedBookIds}
                    onToggleSelect={toggleSelect}
                  />
                ))
              )}
            </>
          )}
        </>
      )}

      {selectMode && (
        <div className="organize-toolbar">
          <span>
            {selectedBookIds.size} selected
          </span>
          <div className="organize-toolbar-actions">
            <Button variant="secondary" onClick={exitSelectMode}>
              Cancel
            </Button>
            <Button disabled={selectedBookIds.size === 0} onClick={() => setSeriesSheetOpen(true)}>
              Set series
            </Button>
          </div>
        </div>
      )}

      <SetSeriesSheet
        open={seriesSheetOpen}
        onClose={() => setSeriesSheetOpen(false)}
        books={selectedBooks}
        onSuccess={exitSelectMode}
      />
    </div>
  );
}
