import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { Button } from '../components/ui/Button';
import { ActionMenu } from '../components/ui/ActionMenu';
import { AuthorRow } from '../components/organize/AuthorRow';
import { PickBookSheet, type PickResult } from '../components/organize/PickBookSheet';
import { PickAuthorSheet } from '../components/organize/PickAuthorSheet';
import { RenameSheet } from '../components/organize/RenameSheet';
import { MatchSheet } from '../components/organize/MatchSheet';
import { ConfirmActionSheet } from '../components/organize/ConfirmActionSheet';
import { OrganizeHelpSheet } from '../components/organize/OrganizeHelpSheet';
import { MoveIcon, LayersIcon, MergeIcon, SearchIcon, ChevronRightIcon } from '../components/organize/icons';
import { RefreshIcon } from '../components/ui/icons';
import { SetSeriesSheet, type SetSeriesBook } from '../components/library/SetSeriesSheet';
import {
  useScannedFiles,
  useApplyChanges,
  useRescanScannedFiles,
  useBackfillMetadata,
} from '../hooks/useOrganize';
import { useLibraryBooks, useLibraries } from '../hooks/useLibraryBooks';
import { Tabs } from '../components/ui/Tabs';
import { buildTree } from '../types/scan';
import type { BookGroup, FileInfo, ChangeDto } from '../types/scan';
import { describeChanges } from '../lib/describeChanges';
import { showToast } from '../lib/toast';
import '../components/organize/organize.css';

type SheetState =
  | { kind: 'rename-author'; author: string }
  | { kind: 'rename-book'; book: BookGroup }
  | { kind: 'rename-file'; file: FileInfo }
  | { kind: 'move-selected' }
  | { kind: 'merge-book'; book: BookGroup }
  | { kind: 'move-book'; book: BookGroup }
  | { kind: 'move-author'; author: string }
  | { kind: 'merge-author'; author: string }
  | { kind: 'match-book'; bookId: number }
  | { kind: 'set-series' }
  | { kind: 'move-selected-books' }
  | { kind: 'merge-selected-books' }
  | { kind: 'move-selected-authors' }
  | null;

export function OrganizePage() {
  const { data: libraries = [] } = useLibraries();
  const [libraryFilter, setLibraryFilter] = useState<string>('all');
  const filterLibraryId = libraryFilter === 'all' ? undefined : Number(libraryFilter);
  const { data, isLoading, isError, refetch } = useScannedFiles(filterLibraryId);
  const applyChanges = useApplyChanges();
  const rescan = useRescanScannedFiles();
  const backfillMetadata = useBackfillMetadata();
  const { data: libraryBooks = [] } = useLibraryBooks();
  const [scanning, setScanning] = useState(false);
  const [selectedFileIds, setSelectedFileIds] = useState<Set<number>>(new Set());
  const [selectedBookIds, setSelectedBookIds] = useState<Set<number>>(new Set());
  const [selectedAuthorIds, setSelectedAuthorIds] = useState<Set<string>>(new Set());
  const [sheet, setSheet] = useState<SheetState>(null);
  const [pendingChanges, setPendingChanges] = useState<ChangeDto[] | null>(null);

  const tree = useMemo(() => (data ? buildTree(data) : []), [data]);

  function toggleSelect(id: number) {
    setSelectedFileIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function toggleSelectBook(bookId: number) {
    setSelectedBookIds((prev) => {
      const next = new Set(prev);
      if (next.has(bookId)) next.delete(bookId);
      else next.add(bookId);
      return next;
    });
  }

  function toggleSelectAuthor(author: string) {
    setSelectedAuthorIds((prev) => {
      const next = new Set(prev);
      if (next.has(author)) next.delete(author);
      else next.add(author);
      return next;
    });
  }

  const treeBooksById = useMemo(
    () => new Map(tree.flatMap((a) => a.books).map((b) => [b.bookId, b])),
    [tree],
  );

  function fileIdsForBooks(bookIds: Set<number>): number[] {
    return Array.from(bookIds).flatMap((id) => treeBooksById.get(id)?.files.map((f) => f.id) ?? []);
  }

  // Sequence prefill comes from the library row when available; tree books that
  // haven't landed in the library cache yet fall back to title-only entries.
  const seriesSheetBooks: SetSeriesBook[] = useMemo(() => {
    return Array.from(selectedBookIds).flatMap((bookId) => {
      const row = libraryBooks.find((b) => b.id === bookId);
      if (row) return [{ id: row.id, title: row.title, sequence: row.series_sequence }];
      const treeBook = treeBooksById.get(bookId);
      return treeBook ? [{ id: bookId, title: treeBook.title, sequence: null }] : [];
    });
  }, [selectedBookIds, libraryBooks, treeBooksById]);

  // Stages changes for review instead of applying them immediately - the
  // confirm sheet shows a dry-run of every change before it hits the server.
  function submitChanges(changes: ChangeDto[]) {
    setPendingChanges(changes);
  }

  function confirmPendingChanges() {
    if (!pendingChanges) return;
    applyChanges.mutate(pendingChanges);
    setSelectedFileIds(new Set());
    setSelectedBookIds(new Set());
    setSelectedAuthorIds(new Set());
    setPendingChanges(null);
  }

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

  function fileIdsForAuthor(author: string): number[] {
    const group = tree.find((a) => a.author === author);
    if (!group) return [];
    return group.books.flatMap((b) => b.files.map((f) => f.id));
  }

  function handlePick(target: PickResult) {
    if (!sheet) return;
    if (sheet.kind === 'move-selected') {
      const file_ids = Array.from(selectedFileIds);
      if (target.kind === 'existing') {
        submitChanges([
          {
            change_type: 'file-move',
            file_ids,
            new_book_id: target.bookId,
            new_author: target.author,
            new_series: target.series,
          },
        ]);
      } else {
        submitChanges([
          {
            change_type: 'file-move',
            file_ids,
            new_book_id: -1,
            new_author: target.author,
            new_series: target.title,
          },
        ]);
      }
    } else if (sheet.kind === 'merge-book') {
      if (target.kind === 'existing') {
        submitChanges([
          {
            change_type: 'merge-title',
            file_ids: sheet.book.files.map((f) => f.id),
            current_book_ids: [sheet.book.bookId],
            new_book_id: target.bookId,
          },
        ]);
      } else {
        submitChanges([
          {
            change_type: 'merge-title',
            file_ids: sheet.book.files.map((f) => f.id),
            current_book_ids: [sheet.book.bookId],
            new_book_id: -1,
            new_author: target.author,
            new_series: target.title,
          },
        ]);
      }
    } else if (sheet.kind === 'merge-selected-books') {
      const file_ids = fileIdsForBooks(selectedBookIds);
      const current_book_ids = Array.from(selectedBookIds);
      if (target.kind === 'existing') {
        submitChanges([
          { change_type: 'merge-title', file_ids, current_book_ids, new_book_id: target.bookId },
        ]);
      } else {
        submitChanges([
          {
            change_type: 'merge-title',
            file_ids,
            current_book_ids,
            new_book_id: -1,
            new_author: target.author,
            new_series: target.title,
          },
        ]);
      }
    }
  }

  function handlePickAuthor(target: string) {
    if (sheet?.kind === 'move-author' || sheet?.kind === 'merge-author') {
      submitChanges([
        { change_type: 'rename', file_ids: fileIdsForAuthor(sheet.author), new_author: target },
      ]);
    } else if (sheet?.kind === 'move-book') {
      submitChanges([
        {
          change_type: 'rename',
          file_ids: sheet.book.files.map((f) => f.id),
          new_author: target,
        },
      ]);
    } else if (sheet?.kind === 'move-selected-books') {
      submitChanges([
        { change_type: 'rename', file_ids: fileIdsForBooks(selectedBookIds), new_author: target },
      ]);
    } else if (sheet?.kind === 'move-selected-authors') {
      const file_ids = Array.from(selectedAuthorIds).flatMap(fileIdsForAuthor);
      submitChanges([{ change_type: 'rename', file_ids, new_author: target }]);
    }
  }

  // Derives the PickAuthorSheet's title/excludeAuthor for whichever sheet.kind
  // currently wants an author picker; null hides the sheet.
  function authorPickConfig(s: SheetState): { title: string; excludeAuthor?: string } | null {
    if (!s) return null;
    switch (s.kind) {
      case 'move-author':
        return { title: 'Move to another author…', excludeAuthor: s.author };
      case 'merge-author':
        return { title: 'Merge into another author…', excludeAuthor: s.author };
      case 'move-book':
        return { title: 'Move to another author…', excludeAuthor: s.book.author };
      case 'move-selected-books':
        return {
          title: `Move ${selectedBookIds.size} book${selectedBookIds.size === 1 ? '' : 's'} to another author…`,
        };
      case 'move-selected-authors':
        return {
          title: `Move ${selectedAuthorIds.size} author${selectedAuthorIds.size === 1 ? '' : 's'}' files to…`,
        };
      default:
        return null;
    }
  }

  // Same idea for the PickBookSheet's title/excludeBookId.
  function bookPickConfig(s: SheetState): { title: string; excludeBookId?: number | number[] } | null {
    if (!s) return null;
    switch (s.kind) {
      case 'move-selected':
        return {
          title: `Move ${selectedFileIds.size} file${selectedFileIds.size === 1 ? '' : 's'} to…`,
        };
      case 'merge-book':
        return { title: 'Merge into…', excludeBookId: s.book.bookId };
      case 'merge-selected-books':
        return {
          title: `Merge ${selectedBookIds.size} book${selectedBookIds.size === 1 ? '' : 's'} into…`,
          excludeBookId: Array.from(selectedBookIds),
        };
      default:
        return null;
    }
  }

  const authorPick = authorPickConfig(sheet);
  const bookPick = bookPickConfig(sheet);

  return (
    <div className="organize-page">
      <div className="organize-header">
        <div>
          <div className="organize-title-row">
            <h1 className="organize-title">Organize</h1>
            <OrganizeHelpSheet />
          </div>
          <p className="organize-subtitle">Rename, move, merge, and match scanned files</p>
          {backfillMetadata.data && (
            <p className="organize-subtitle">
              Metadata: checked {backfillMetadata.data.checked}, updated{' '}
              {backfillMetadata.data.updated}, skipped {backfillMetadata.data.skipped}.
            </p>
          )}
        </div>
        <div className="organize-actions">
          <Button variant="secondary" onClick={handleRescan} disabled={scanning}>
            <RefreshIcon size={16} className={scanning ? 'spin' : undefined} /> {scanning ? 'Scanning…' : 'Rescan'}
          </Button>
          <ActionMenu
            items={[
              {
                label: backfillMetadata.isPending ? 'Fetching metadata…' : 'Fetch missing metadata',
                icon: <SearchIcon size={16} />,
                onClick: () => backfillMetadata.mutate(),
                disabled: backfillMetadata.isPending,
              },
            ]}
          />
          <Link className="organize-back-link" to="/">
            <ChevronRightIcon size={16} className="rotate-180" /> Back to library
          </Link>
        </div>
      </div>

      {libraries.length > 1 && (
        <Tabs
          tabs={[
            { id: 'all', label: 'All Libraries' },
            ...libraries.map((l) => ({ id: String(l.id), label: l.name })),
          ]}
          activeId={libraryFilter}
          onChange={setLibraryFilter}
        />
      )}

      {isLoading && <div className="organize-state">Loading scanned files…</div>}
      {isError && (
        <div className="organize-state">
          Couldn't load scanned files.
          <Button variant="primary" onClick={() => refetch()}>
            Try again
          </Button>
        </div>
      )}
      {!isLoading && !isError && tree.length === 0 && (
        <div className="organize-state">No scanned files found. Try a rescan.</div>
      )}

      <div className="author-list">
        {tree.map((group) => (
          <AuthorRow
            key={group.author}
            group={group}
            selectedFileIds={selectedFileIds}
            selectedBookIds={selectedBookIds}
            selectedAuthorIds={selectedAuthorIds}
            onToggleSelect={toggleSelect}
            onToggleSelectBook={toggleSelectBook}
            onToggleSelectAuthor={toggleSelectAuthor}
            onRenameAuthor={(author) => setSheet({ kind: 'rename-author', author })}
            onMoveAuthor={(author) => setSheet({ kind: 'move-author', author })}
            onMergeAuthor={(author) => setSheet({ kind: 'merge-author', author })}
            onRenameBook={(book) => setSheet({ kind: 'rename-book', book })}
            onRenameFile={(file) => setSheet({ kind: 'rename-file', file })}
            onMoveBook={(book) => setSheet({ kind: 'move-book', book })}
            onMergeBook={(book) => setSheet({ kind: 'merge-book', book })}
            onMatchBook={(book) => setSheet({ kind: 'match-book', bookId: book.bookId })}
          />
        ))}
      </div>

      {(selectedFileIds.size > 0 || selectedBookIds.size > 0 || selectedAuthorIds.size > 0) && (
        <div className="organize-toolbar">
          <span>
            {selectedFileIds.size > 0 &&
              `${selectedFileIds.size} file${selectedFileIds.size === 1 ? '' : 's'}`}
            {selectedFileIds.size > 0 && (selectedBookIds.size > 0 || selectedAuthorIds.size > 0) && ', '}
            {selectedBookIds.size > 0 &&
              `${selectedBookIds.size} book${selectedBookIds.size === 1 ? '' : 's'}`}
            {selectedBookIds.size > 0 && selectedAuthorIds.size > 0 && ', '}
            {selectedAuthorIds.size > 0 &&
              `${selectedAuthorIds.size} author${selectedAuthorIds.size === 1 ? '' : 's'}`}
            {' selected'}
          </span>
          <div className="organize-toolbar-actions">
            <Button
              variant="ghost"
              onClick={() => {
                setSelectedFileIds(new Set());
                setSelectedBookIds(new Set());
                setSelectedAuthorIds(new Set());
              }}
            >
              Clear
            </Button>
            {selectedFileIds.size > 0 && (
              <Button variant="primary" onClick={() => setSheet({ kind: 'move-selected' })}>
                <MoveIcon size={14} /> Move files
              </Button>
            )}
            {selectedBookIds.size > 0 && (
              <Button variant="primary" onClick={() => setSheet({ kind: 'move-selected-books' })}>
                <MoveIcon size={14} /> Move to author
              </Button>
            )}
            {selectedBookIds.size > 0 && (
              <Button variant="primary" onClick={() => setSheet({ kind: 'merge-selected-books' })}>
                <MergeIcon size={14} /> Merge
              </Button>
            )}
            {selectedBookIds.size > 0 && (
              <Button variant="primary" onClick={() => setSheet({ kind: 'set-series' })}>
                <LayersIcon size={14} /> Set series
              </Button>
            )}
            {selectedAuthorIds.size > 0 && (
              <Button variant="primary" onClick={() => setSheet({ kind: 'move-selected-authors' })}>
                <MoveIcon size={14} /> Move authors
              </Button>
            )}
          </div>
        </div>
      )}

      <PickBookSheet
        open={bookPick !== null}
        onClose={() => setSheet(null)}
        title={bookPick?.title ?? ''}
        tree={tree}
        excludeBookId={bookPick?.excludeBookId}
        allowCreateNew
        onPick={handlePick}
      />

      <PickAuthorSheet
        open={authorPick !== null}
        onClose={() => setSheet(null)}
        title={authorPick?.title ?? ''}
        tree={tree}
        excludeAuthor={authorPick?.excludeAuthor}
        onPick={handlePickAuthor}
      />

      <RenameSheet
        open={sheet?.kind === 'rename-author'}
        onClose={() => setSheet(null)}
        scope="author"
        tree={tree}
        initialAuthor={sheet?.kind === 'rename-author' ? sheet.author : undefined}
        onSubmit={(values) => {
          if (sheet?.kind !== 'rename-author' || !values.author) return;
          submitChanges([
            { change_type: 'rename', file_ids: fileIdsForAuthor(sheet.author), new_author: values.author },
          ]);
        }}
      />

      <RenameSheet
        open={sheet?.kind === 'rename-book'}
        onClose={() => setSheet(null)}
        scope="book"
        tree={tree}
        initialAuthor={sheet?.kind === 'rename-book' ? sheet.book.author : undefined}
        initialTitle={sheet?.kind === 'rename-book' ? sheet.book.title : undefined}
        onSubmit={(values) => {
          if (sheet?.kind !== 'rename-book') return;
          const change: ChangeDto = {
            change_type: 'rename',
            file_ids: sheet.book.files.map((f) => f.id),
          };
          if (values.author) change.new_author = values.author;
          if (values.title) change.new_series = values.title;
          if (!change.new_author && !change.new_series) return;
          submitChanges([change]);
        }}
      />

      <RenameSheet
        open={sheet?.kind === 'rename-file'}
        onClose={() => setSheet(null)}
        scope="file"
        tree={tree}
        initialTitle={sheet?.kind === 'rename-file' ? sheet.file.file_name : undefined}
        onSubmit={(values) => {
          if (sheet?.kind !== 'rename-file' || !values.title) return;
          submitChanges([
            { change_type: 'rename', file_ids: [sheet.file.id], new_filetitle: values.title },
          ]);
        }}
      />

      <ConfirmActionSheet
        open={pendingChanges !== null}
        lines={pendingChanges ? describeChanges(pendingChanges, tree) : []}
        onCancel={() => setPendingChanges(null)}
        onConfirm={confirmPendingChanges}
      />

      <MatchSheet
        open={sheet?.kind === 'match-book'}
        onClose={() => setSheet(null)}
        bookId={sheet?.kind === 'match-book' ? sheet.bookId : null}
      />

      <SetSeriesSheet
        open={sheet?.kind === 'set-series'}
        onClose={() => setSheet(null)}
        books={seriesSheetBooks}
        onSuccess={() => setSelectedBookIds(new Set())}
      />
    </div>
  );
}
