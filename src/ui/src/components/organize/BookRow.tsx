import { useState } from 'react';
import type { BookGroup, FileInfo } from '../../types/scan';
import { FileRow } from './FileRow';
import { ChevronRightIcon, PencilIcon, MoveIcon, MergeIcon, SearchIcon } from './icons';
import { ReorderIcon } from '../ui/icons';
import { ReorderChaptersSheet } from '../ui/ReorderChaptersSheet';

interface BookRowProps {
  book: BookGroup;
  selectedFileIds: Set<number>;
  selectedBookIds: Set<number>;
  onToggleSelect: (id: number) => void;
  onToggleSelectBook: (bookId: number) => void;
  onRenameBook: (book: BookGroup) => void;
  onRenameFile: (file: FileInfo) => void;
  onMoveBook: (book: BookGroup) => void;
  onMergeBook: (book: BookGroup) => void;
  onMatchBook: (book: BookGroup) => void;
}

export function BookRow({
  book,
  selectedFileIds,
  selectedBookIds,
  onToggleSelect,
  onToggleSelectBook,
  onRenameBook,
  onRenameFile,
  onMoveBook,
  onMergeBook,
  onMatchBook,
}: BookRowProps) {
  const [expanded, setExpanded] = useState(false);
  const [reordering, setReordering] = useState(false);
  const selectionActive = selectedBookIds.size > 0;

  return (
    <div className="book-row">
      <div className="book-header" onClick={() => setExpanded((e) => !e)}>
        <input
          type="checkbox"
          className="file-checkbox"
          aria-label={`Select ${book.title}`}
          checked={selectedBookIds.has(book.bookId)}
          onChange={() => onToggleSelectBook(book.bookId)}
          onClick={(e) => e.stopPropagation()}
        />
        <ChevronRightIcon className={`book-chevron ${expanded ? 'expanded' : ''}`} />
        <div className="book-header-text">
          <div className="book-title">{book.title}</div>
          <div className="book-meta">
            {book.files.length} {book.files.length === 1 ? 'file' : 'files'}
          </div>
        </div>
      </div>
      <div className="book-actions" onClick={(e) => e.stopPropagation()}>
        <button
          className="icon-btn"
          aria-label="Rename or move to a different author"
          title="Rename or move to a different author"
          onClick={() => onRenameBook(book)}
        >
          <PencilIcon />
        </button>
        <button
          className="icon-btn"
          aria-label="Move to another author"
          title={selectionActive ? 'Clear selection or use the bulk Move action below' : 'Move to another author'}
          disabled={selectionActive}
          onClick={() => onMoveBook(book)}
        >
          <MoveIcon />
        </button>
        <button
          className="icon-btn"
          aria-label="Merge into another book"
          title={selectionActive ? 'Clear selection or use the bulk Merge action below' : 'Merge into another book'}
          disabled={selectionActive}
          onClick={() => onMergeBook(book)}
        >
          <MergeIcon />
        </button>
        <button
          className="icon-btn"
          aria-label="Match metadata"
          title="Match metadata"
          onClick={() => onMatchBook(book)}
        >
          <SearchIcon />
        </button>
        <button
          className="icon-btn"
          aria-label="Reorder chapters"
          title="Reorder chapters"
          onClick={() => setReordering(true)}
        >
          <ReorderIcon />
        </button>
      </div>

      <ReorderChaptersSheet
        open={reordering}
        onClose={() => setReordering(false)}
        bookId={book.bookId}
      />

      {expanded && (
        <div className="file-list">
          {book.files.map((f) => (
            <FileRow
              key={f.id}
              file={f}
              selected={selectedFileIds.has(f.id)}
              onToggleSelect={onToggleSelect}
              onRename={onRenameFile}
            />
          ))}
        </div>
      )}
    </div>
  );
}
