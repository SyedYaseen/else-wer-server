import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { fileMetadata, coverUrl } from '../api/books';
import { getBookProgress } from '../api/progress';
import { resolveResumePoint, reconcileProgress, getLocalProgress } from '../lib/playerResume';
import type { AudioBookRow, FileMetadata } from '../types/book';
import { loadBook } from '../player/engine';
import { useLibraryBooks, useDeleteBook, LIBRARY_KEYS } from '../hooks/useLibraryBooks';
import { useScannedFiles, useApplyChanges } from '../hooks/useOrganize';
import { useAuthStore } from '../store/auth';
import { buildTree } from '../types/scan';
import type { ChangeDto } from '../types/scan';
import { formatDuration } from '../lib/format';
import { Button } from '../components/ui/Button';
import { ProgressBar } from '../components/ui/ProgressBar';
import { ActionMenu } from '../components/ui/ActionMenu';
import { BottomSheet } from '../components/ui/BottomSheet';
import { isBookDownloaded, getOfflineBookData } from '../offline/storage';
import { startDownload, useDownloadStore } from '../offline/downloadStore';
import { MatchSheet } from '../components/organize/MatchSheet';
import { RenameSheet } from '../components/organize/RenameSheet';
import { PickBookSheet, type PickResult } from '../components/organize/PickBookSheet';
import {
  SearchIcon,
  PencilIcon,
  MoveIcon,
  MergeIcon,
  LayersIcon,
  ChevronRightIcon,
} from '../components/organize/icons';
import { PlayIcon } from '../components/player/icons';
import { DownloadIcon } from '../components/ui/icons';
import { SetSeriesSheet } from '../components/library/SetSeriesSheet';
import '../components/library/BookDetailPage.css';
import '../components/organize/organize.css';

type DetailSheet = 'rename' | 'move' | 'merge' | 'series' | 'move-files' | null;

export function BookDetailPage() {
  const { id } = useParams<{ id: string }>();
  const bookId = Number(id);
  const navigate = useNavigate();
  const [downloaded, setDownloaded] = useState(false);
  const [matchOpen, setMatchOpen] = useState(false);
  const [sheet, setSheet] = useState<DetailSheet>(null);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const [selectedFileIds, setSelectedFileIds] = useState<Set<number>>(new Set());
  const isAdmin = useAuthStore((s) => s.isAdmin());
  const deleteBook = useDeleteBook();

  const { data: books = [], isLoading: booksLoading } = useLibraryBooks();

  const { data: files_, isLoading, isError } = useQuery({
    queryKey: [...LIBRARY_KEYS.books, bookId, 'files'],
    queryFn: () => fileMetadata(bookId),
    enabled: Number.isFinite(bookId),
  });

  // Offline fallback: if a book was previously downloaded, its metadata
  // snapshot lets this page (and Play) still work with no network at all.
  // undefined = check pending, null = no snapshot — distinguished so the
  // "Book not found" state doesn't flash while the lookup is in flight.
  const [offlineData, setOfflineData] = useState<{ book: AudioBookRow; files: FileMetadata[] } | null | undefined>(
    undefined,
  );
  useEffect(() => {
    if (!Number.isFinite(bookId)) return;
    getOfflineBookData(bookId)
      .then(setOfflineData)
      .catch(() => setOfflineData(null));
  }, [bookId]);

  const book = books.find((b) => b.id === bookId) ?? offlineData?.book;
  const files = useMemo(() => files_ ?? offlineData?.files ?? [], [files_, offlineData]);

  const dl = useDownloadStore((s) => s.byBook[bookId]);
  const downloading = dl?.status === 'downloading';

  const { data: scanned } = useScannedFiles();
  const tree = useMemo(() => (scanned ? buildTree(scanned) : []), [scanned]);
  const bookGroup = useMemo(
    () => tree.flatMap((a) => a.books).find((b) => b.bookId === bookId),
    [tree, bookId],
  );
  const fileIds = useMemo(() => bookGroup?.files.map((f) => f.id) ?? [], [bookGroup]);
  const filePathToScanId = useMemo(() => {
    const map = new Map<string, number>();
    for (const f of bookGroup?.files ?? []) map.set(f.file_path, f.id);
    return map;
  }, [bookGroup]);
  const applyChanges = useApplyChanges();

  function toggleSelectFile(id: number) {
    setSelectedFileIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function handlePick(target: PickResult) {
    if (sheet === 'move') {
      if (target.kind === 'existing') {
        applyChanges.mutate([
          {
            change_type: 'file-move',
            file_ids: fileIds,
            new_book_id: target.bookId,
            new_author: target.author,
            new_series: target.series,
          },
        ]);
      } else {
        applyChanges.mutate([
          {
            change_type: 'file-move',
            file_ids: fileIds,
            new_book_id: -1,
            new_author: target.author,
            new_series: target.title,
          },
        ]);
      }
    } else if (sheet === 'merge' && target.kind === 'existing') {
      applyChanges.mutate([
        {
          change_type: 'merge-title',
          file_ids: fileIds,
          current_book_ids: [bookId],
          new_book_id: target.bookId,
        },
      ]);
      navigate('/');
    } else if (sheet === 'move-files') {
      const selectedScanIds = files
        .filter((f) => selectedFileIds.has(f.id))
        .map((f) => filePathToScanId.get(f.file_path))
        .filter((id): id is number => id !== undefined);
      if (selectedScanIds.length > 0) {
        if (target.kind === 'existing') {
          applyChanges.mutate([
            {
              change_type: 'file-move',
              file_ids: selectedScanIds,
              new_book_id: target.bookId,
              new_author: target.author,
              new_series: target.series,
            },
          ]);
        } else {
          applyChanges.mutate([
            {
              change_type: 'file-move',
              file_ids: selectedScanIds,
              new_book_id: -1,
              new_author: target.author,
              new_series: target.title,
            },
          ]);
        }
        if (selectedFileIds.size >= files.length) navigate('/');
      }
      setSelectedFileIds(new Set());
    }
    setSheet(null);
  }

  const dlStatus = dl?.status;
  useEffect(() => {
    if (!book || files.length === 0 || dlStatus === 'downloading') return;
    let cancelled = false;
    isBookDownloaded(book.id, files).then((result) => {
      if (!cancelled) setDownloaded(result);
    });
    return () => {
      cancelled = true;
    };
  }, [book, files, dlStatus]);

  function handleDownload() {
    if (!book || files.length === 0) return;
    void startDownload(book, files);
  }

  async function handleDeleteBook() {
    if (!book) return;
    await deleteBook.mutateAsync(book.id);
    navigate('/');
  }

  async function handlePlay() {
    if (!book) return;
    let resume: ReturnType<typeof resolveResumePoint>;
    try {
      const progress = await getBookProgress(book.id);
      resume = reconcileProgress(files, progress, getLocalProgress(book.id));
    } catch {
      resume = resolveResumePoint(files, getLocalProgress(book.id));
    }
    loadBook(book, files, resume);
    navigate(`/player/${book.id}`);
  }

  if (!book) {
    // Don't flash "Book not found" while either source is still resolving.
    const pending = booksLoading || offlineData === undefined;
    return (
      <div className="book-detail-page">
        <Link to="/" className="book-detail-back">
  <ChevronRightIcon size={16} className="rotate-180" /> Back to library
        </Link>
        {!pending && <p>Book not found.</p>}
      </div>
    );
  }

  const cover = coverUrl(book.cover_art);

  return (
    <div className="book-detail-page">
      <Link to="/" className="book-detail-back">
<ChevronRightIcon size={16} className="rotate-180" /> Back to library
      </Link>

      <div className="book-detail-header">
        {cover ? (
          <img className="book-detail-cover" src={cover} alt="" />
        ) : (
          <div className="book-detail-cover-placeholder">{book.title?.charAt(0).toUpperCase() ?? '?'}</div>
        )}
        <div>
          <h1 className="book-detail-title">{book.title}</h1>
          <p className="book-detail-author">{book.author}</p>
          {book.series_name ? (
            <p className="book-detail-meta">
              {book.series_name}
              {book.series_sequence ? ` · Book ${book.series_sequence}` : ''}
            </p>
          ) : (
            book.series && <p className="book-detail-meta">{book.series}</p>
          )}
          {book.narrated_by && <p className="book-detail-meta">Narrated by {book.narrated_by}</p>}
          <div className="book-detail-actions">
            <Button variant="primary" onClick={handlePlay} disabled={files.length === 0}>
              <PlayIcon size={16} /> Play
            </Button>
            <Button variant="secondary" onClick={handleDownload} disabled={downloading || files.length === 0}>
              <DownloadIcon size={16} />{' '}
              {downloading ? 'Downloading…' : downloaded || dl?.status === 'done' ? 'Downloaded' : 'Download'}
            </Button>
            <ActionMenu
              items={[
                { label: 'Recheck metadata', icon: <SearchIcon size={16} />, onClick: () => setMatchOpen(true) },
                { label: 'Rename', icon: <PencilIcon size={16} />, onClick: () => setSheet('rename') },
                { label: 'Move', icon: <MoveIcon size={16} />, onClick: () => setSheet('move') },
                { label: 'Merge', icon: <MergeIcon size={16} />, onClick: () => setSheet('merge') },
                { label: 'Set series', icon: <LayersIcon size={16} />, onClick: () => setSheet('series') },
                isAdmin && { label: 'Delete book', onClick: () => setDeleteConfirmOpen(true) },
              ]}
            />
          </div>
          {dl?.status === 'downloading' && (
            <ProgressBar value={dl.fraction} className="book-detail-download-progress" />
          )}
          {dl?.status === 'error' && <p className="book-detail-download-error">Download failed: {dl.message}</p>}
        </div>
      </div>

      {book.description && (
        <>
          <h2 className="book-detail-section-title">Description</h2>
          <p className="book-detail-description">{book.description}</p>
        </>
      )}

      <h2 className="book-detail-section-title">Files</h2>
      {isLoading && <p>Loading files…</p>}
      {isError && <p>Couldn't load files for this book.</p>}
      <div className="file-list">
        {files.map((file) => (
          <div className="card file-list-item" key={file.id}>
            {files.length > 1 && (
              <input
                type="checkbox"
                className="file-checkbox"
                checked={selectedFileIds.has(file.id)}
                onChange={() => toggleSelectFile(file.id)}
              />
            )}
            <span className="file-list-name">{file.file_name}</span>
            <span className="file-list-duration">{formatDuration(file.duration)}</span>
          </div>
        ))}
      </div>

      {selectedFileIds.size > 0 && (
        <div className="organize-toolbar">
          <span>
            {selectedFileIds.size} file{selectedFileIds.size === 1 ? '' : 's'} selected
          </span>
          <div className="organize-toolbar-actions">
            <Button variant="ghost" onClick={() => setSelectedFileIds(new Set())}>
              Clear
            </Button>
            <Button variant="primary" onClick={() => setSheet('move-files')}>
              <MoveIcon size={14} /> Move
            </Button>
          </div>
        </div>
      )}

      <MatchSheet open={matchOpen} onClose={() => setMatchOpen(false)} bookId={book.id} />

      <RenameSheet
        open={sheet === 'rename'}
        onClose={() => setSheet(null)}
        scope="book"
        initialAuthor={book.author}
        initialTitle={book.title}
        onSubmit={(values) => {
          const change: ChangeDto = { change_type: 'rename', file_ids: fileIds };
          if (values.author) change.new_author = values.author;
          if (values.title) change.new_series = values.title;
          if (!change.new_author && !change.new_series) return;
          applyChanges.mutate([change]);
        }}
      />

      <PickBookSheet
        open={sheet === 'move'}
        onClose={() => setSheet(null)}
        title="Move to…"
        tree={tree}
        excludeBookId={bookId}
        allowCreateNew
        onPick={handlePick}
      />

      <PickBookSheet
        open={sheet === 'merge'}
        onClose={() => setSheet(null)}
        title="Merge into…"
        tree={tree}
        excludeBookId={bookId}
        onPick={handlePick}
      />

      <PickBookSheet
        open={sheet === 'move-files'}
        onClose={() => setSheet(null)}
        title={`Move ${selectedFileIds.size} file${selectedFileIds.size === 1 ? '' : 's'} to…`}
        tree={tree}
        excludeBookId={bookId}
        allowCreateNew
        onPick={handlePick}
      />

      <SetSeriesSheet
        open={sheet === 'series'}
        onClose={() => setSheet(null)}
        books={[{ id: book.id, title: book.title, sequence: book.series_sequence }]}
      />

      <BottomSheet open={deleteConfirmOpen} onClose={() => setDeleteConfirmOpen(false)}>
        <h3 className="sheet-title">Delete book</h3>
        <p className="book-detail-delete-confirm-text">
          Delete <strong>{book.title}</strong> and its files? This can't be undone.
        </p>
        <div className="book-detail-delete-confirm-actions">
          <Button variant="ghost" onClick={() => setDeleteConfirmOpen(false)}>
            Cancel
          </Button>
          <Button variant="destructive" onClick={handleDeleteBook} disabled={deleteBook.isPending}>
            {deleteBook.isPending ? 'Deleting…' : 'Delete'}
          </Button>
        </div>
      </BottomSheet>
    </div>
  );
}
