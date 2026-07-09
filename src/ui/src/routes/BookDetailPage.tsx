import { useMemo, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { fileMetadata, coverUrl, downloadBook } from '../api/books';
import { getBookProgress } from '../api/progress';
import { resolveResumePoint } from '../lib/playerResume';
import { loadBook } from '../player/engine';
import { useLibraryBooks, LIBRARY_KEYS } from '../hooks/useLibraryBooks';
import { useScannedFiles, useApplyChanges } from '../hooks/useOrganize';
import { buildTree } from '../types/scan';
import type { ChangeDto } from '../types/scan';
import { formatDuration } from '../lib/format';
import { Button } from '../components/ui/Button';
import { ActionMenu } from '../components/ui/ActionMenu';
import { MatchSheet } from '../components/organize/MatchSheet';
import { RenameSheet } from '../components/organize/RenameSheet';
import { PickBookSheet, type PickResult } from '../components/organize/PickBookSheet';
import { SearchIcon, PencilIcon, MoveIcon, MergeIcon, LayersIcon } from '../components/organize/icons';
import { SetSeriesSheet } from '../components/library/SetSeriesSheet';
import '../components/library/BookDetailPage.css';

type DetailSheet = 'rename' | 'move' | 'merge' | 'series' | null;

export function BookDetailPage() {
  const { id } = useParams<{ id: string }>();
  const bookId = Number(id);
  const navigate = useNavigate();
  const [downloading, setDownloading] = useState(false);
  const [matchOpen, setMatchOpen] = useState(false);
  const [sheet, setSheet] = useState<DetailSheet>(null);

  const { data: books = [] } = useLibraryBooks();
  const book = books.find((b) => b.id === bookId);

  const { data: files = [], isLoading, isError } = useQuery({
    queryKey: [...LIBRARY_KEYS.books, bookId, 'files'],
    queryFn: () => fileMetadata(bookId),
    enabled: Number.isFinite(bookId),
  });

  const { data: scanned } = useScannedFiles();
  const tree = useMemo(() => (scanned ? buildTree(scanned) : []), [scanned]);
  const bookGroup = useMemo(
    () => tree.flatMap((a) => a.books).find((b) => b.bookId === bookId),
    [tree, bookId],
  );
  const fileIds = useMemo(() => bookGroup?.files.map((f) => f.id) ?? [], [bookGroup]);
  const applyChanges = useApplyChanges();

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
    }
    setSheet(null);
  }

  async function handleDownload() {
    if (!book) return;
    setDownloading(true);
    try {
      await downloadBook(book.id);
    } finally {
      setDownloading(false);
    }
  }

  async function handlePlay() {
    if (!book) return;
    const progress = await getBookProgress(book.id);
    const resume = resolveResumePoint(files, progress);
    loadBook(book, files, resume);
    navigate(`/player/${book.id}`);
  }

  if (!book) {
    return (
      <div className="book-detail-page">
        <Link to="/" className="book-detail-back">
          ← Back to library
        </Link>
        <p>Book not found.</p>
      </div>
    );
  }

  const cover = coverUrl(book.cover_art);

  return (
    <div className="book-detail-page">
      <Link to="/" className="book-detail-back">
        ← Back to library
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
              Play
            </Button>
            <Button variant="secondary" onClick={handleDownload} disabled={downloading}>
              {downloading ? 'Downloading…' : 'Download'}
            </Button>
            <ActionMenu
              items={[
                { label: 'Recheck metadata', icon: <SearchIcon size={16} />, onClick: () => setMatchOpen(true) },
                { label: 'Rename', icon: <PencilIcon size={16} />, onClick: () => setSheet('rename') },
                { label: 'Move', icon: <MoveIcon size={16} />, onClick: () => setSheet('move') },
                { label: 'Merge', icon: <MergeIcon size={16} />, onClick: () => setSheet('merge') },
                { label: 'Set series', icon: <LayersIcon size={16} />, onClick: () => setSheet('series') },
              ]}
            />
          </div>
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
            <span className="file-list-name">{file.file_name}</span>
            <span className="file-list-duration">{formatDuration(file.duration)}</span>
          </div>
        ))}
      </div>

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

      <SetSeriesSheet
        open={sheet === 'series'}
        onClose={() => setSheet(null)}
        books={[{ id: book.id, title: book.title, sequence: book.series_sequence }]}
      />
    </div>
  );
}
