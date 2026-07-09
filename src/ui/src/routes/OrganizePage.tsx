import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { Button } from '../components/ui/Button';
import { ActionMenu } from '../components/ui/ActionMenu';
import { AuthorRow } from '../components/organize/AuthorRow';
import { PickBookSheet, type PickResult } from '../components/organize/PickBookSheet';
import { RenameSheet } from '../components/organize/RenameSheet';
import { MatchSheet } from '../components/organize/MatchSheet';
import { MoveIcon, LayersIcon } from '../components/organize/icons';
import { SetSeriesSheet, type SetSeriesBook } from '../components/library/SetSeriesSheet';
import {
  useScannedFiles,
  useApplyChanges,
  useRescanScannedFiles,
  useBackfillMetadata,
} from '../hooks/useOrganize';
import { useLibraryBooks } from '../hooks/useLibraryBooks';
import { buildTree } from '../types/scan';
import type { BookGroup, FileInfo, ChangeDto } from '../types/scan';
import '../components/organize/organize.css';

type SheetState =
  | { kind: 'rename-author'; author: string }
  | { kind: 'rename-book'; book: BookGroup }
  | { kind: 'rename-file'; file: FileInfo }
  | { kind: 'move-selected' }
  | { kind: 'merge-book'; book: BookGroup }
  | { kind: 'match-book'; bookId: number }
  | { kind: 'set-series' }
  | null;

export function OrganizePage() {
  const { data, isLoading, isError, refetch } = useScannedFiles();
  const applyChanges = useApplyChanges();
  const rescan = useRescanScannedFiles();
  const backfillMetadata = useBackfillMetadata();
  const { data: libraryBooks = [] } = useLibraryBooks();
  const [scanning, setScanning] = useState(false);
  const [selectedFileIds, setSelectedFileIds] = useState<Set<number>>(new Set());
  const [selectedBookIds, setSelectedBookIds] = useState<Set<number>>(new Set());
  const [sheet, setSheet] = useState<SheetState>(null);

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

  // Sequence prefill comes from the library row when available; tree books that
  // haven't landed in the library cache yet fall back to title-only entries.
  const seriesSheetBooks: SetSeriesBook[] = useMemo(() => {
    const treeBooks = new Map(tree.flatMap((a) => a.books).map((b) => [b.bookId, b]));
    return Array.from(selectedBookIds).flatMap((bookId) => {
      const row = libraryBooks.find((b) => b.id === bookId);
      if (row) return [{ id: row.id, title: row.title, sequence: row.series_sequence }];
      const treeBook = treeBooks.get(bookId);
      return treeBook ? [{ id: bookId, title: treeBook.title, sequence: null }] : [];
    });
  }, [selectedBookIds, libraryBooks, tree]);

  function submitChanges(changes: ChangeDto[]) {
    applyChanges.mutate(changes);
    setSelectedFileIds(new Set());
  }

  async function handleRescan() {
    setScanning(true);
    try {
      await rescan();
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
    } else if (sheet.kind === 'merge-book' && target.kind === 'existing') {
      submitChanges([
        {
          change_type: 'merge-title',
          file_ids: sheet.book.files.map((f) => f.id),
          current_book_ids: [sheet.book.bookId],
          new_book_id: target.bookId,
        },
      ]);
    }
  }

  return (
    <div className="organize-page">
      <div className="organize-header">
        <div>
          <h1 className="organize-title">Organize</h1>
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
            {scanning ? 'Scanning…' : 'Rescan'}
          </Button>
          <ActionMenu
            items={[
              {
                label: backfillMetadata.isPending ? 'Fetching metadata…' : 'Fetch missing metadata',
                onClick: () => backfillMetadata.mutate(),
                disabled: backfillMetadata.isPending,
              },
            ]}
          />
          <Link className="organize-back-link" to="/">
            Back to library
          </Link>
        </div>
      </div>

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
            onToggleSelect={toggleSelect}
            onToggleSelectBook={toggleSelectBook}
            onRenameAuthor={(author) => setSheet({ kind: 'rename-author', author })}
            onRenameBook={(book) => setSheet({ kind: 'rename-book', book })}
            onRenameFile={(file) => setSheet({ kind: 'rename-file', file })}
            onMergeBook={(book) => setSheet({ kind: 'merge-book', book })}
            onMatchBook={(book) => setSheet({ kind: 'match-book', bookId: book.bookId })}
          />
        ))}
      </div>

      {(selectedFileIds.size > 0 || selectedBookIds.size > 0) && (
        <div className="organize-toolbar">
          <span>
            {selectedFileIds.size > 0 &&
              `${selectedFileIds.size} file${selectedFileIds.size === 1 ? '' : 's'}`}
            {selectedFileIds.size > 0 && selectedBookIds.size > 0 && ', '}
            {selectedBookIds.size > 0 &&
              `${selectedBookIds.size} book${selectedBookIds.size === 1 ? '' : 's'}`}
            {' selected'}
          </span>
          <div className="organize-toolbar-actions">
            <Button
              variant="ghost"
              onClick={() => {
                setSelectedFileIds(new Set());
                setSelectedBookIds(new Set());
              }}
            >
              Clear
            </Button>
            {selectedFileIds.size > 0 && (
              <Button variant="primary" onClick={() => setSheet({ kind: 'move-selected' })}>
                <MoveIcon size={14} /> Move
              </Button>
            )}
            {selectedBookIds.size > 0 && (
              <Button variant="primary" onClick={() => setSheet({ kind: 'set-series' })}>
                <LayersIcon size={14} /> Set series
              </Button>
            )}
          </div>
        </div>
      )}

      <PickBookSheet
        open={sheet?.kind === 'move-selected'}
        onClose={() => setSheet(null)}
        title={`Move ${selectedFileIds.size} file${selectedFileIds.size === 1 ? '' : 's'} to…`}
        tree={tree}
        allowCreateNew
        onPick={handlePick}
      />

      <PickBookSheet
        open={sheet?.kind === 'merge-book'}
        onClose={() => setSheet(null)}
        title="Merge into…"
        tree={tree}
        excludeBookId={sheet?.kind === 'merge-book' ? sheet.book.bookId : undefined}
        onPick={handlePick}
      />

      <RenameSheet
        open={sheet?.kind === 'rename-author'}
        onClose={() => setSheet(null)}
        scope="author"
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
        initialTitle={sheet?.kind === 'rename-file' ? sheet.file.file_name : undefined}
        onSubmit={(values) => {
          if (sheet?.kind !== 'rename-file' || !values.title) return;
          submitChanges([
            { change_type: 'rename', file_ids: [sheet.file.id], new_filetitle: values.title },
          ]);
        }}
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
