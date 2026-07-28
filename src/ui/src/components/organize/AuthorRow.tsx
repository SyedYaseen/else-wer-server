import { useState } from 'react';
import type { AuthorGroup, BookGroup, FileInfo } from '../../types/scan';
import { BookRow } from './BookRow';
import { ChevronRightIcon, PencilIcon, MoveIcon, MergeIcon } from './icons';

interface AuthorRowProps {
  group: AuthorGroup;
  selectedFileIds: Set<number>;
  selectedBookIds: Set<number>;
  selectedAuthorIds: Set<string>;
  onToggleSelect: (id: number) => void;
  onToggleSelectBook: (bookId: number) => void;
  onToggleSelectAuthor: (author: string) => void;
  onRenameAuthor: (author: string) => void;
  onMoveAuthor: (author: string) => void;
  onMergeAuthor: (author: string) => void;
  onRenameBook: (book: BookGroup) => void;
  onRenameFile: (file: FileInfo) => void;
  onMoveBook: (book: BookGroup) => void;
  onMergeBook: (book: BookGroup) => void;
  onMatchBook: (book: BookGroup) => void;
}

export function AuthorRow({
  group,
  selectedFileIds,
  selectedBookIds,
  selectedAuthorIds,
  onToggleSelect,
  onToggleSelectBook,
  onToggleSelectAuthor,
  onRenameAuthor,
  onMoveAuthor,
  onMergeAuthor,
  onRenameBook,
  onRenameFile,
  onMoveBook,
  onMergeBook,
  onMatchBook,
}: AuthorRowProps) {
  const [expanded, setExpanded] = useState(false);
  const selectionActive = selectedAuthorIds.size > 0;

  return (
    <div className="author-row">
      <div className="author-header" onClick={() => setExpanded((e) => !e)}>
        <input
          type="checkbox"
          className="file-checkbox"
          aria-label={`Select ${group.author}`}
          checked={selectedAuthorIds.has(group.author)}
          onChange={() => onToggleSelectAuthor(group.author)}
          onClick={(e) => e.stopPropagation()}
        />
        <ChevronRightIcon className={`author-chevron ${expanded ? 'expanded' : ''}`} />
        <span className="author-name">{group.author}</span>
        <span className="author-book-count">
          {group.books.length} {group.books.length === 1 ? 'book' : 'books'}
        </span>
        <button
          className="icon-btn"
          aria-label="Rename author"
          title="Rename author"
          onClick={(e) => {
            e.stopPropagation();
            onRenameAuthor(group.author);
          }}
        >
          <PencilIcon size={14} />
        </button>
        <button
          className="icon-btn"
          aria-label="Move to another author"
          title={selectionActive ? 'Clear selection or use the bulk Move action below' : 'Move to another author'}
          disabled={selectionActive}
          onClick={(e) => {
            e.stopPropagation();
            onMoveAuthor(group.author);
          }}
        >
          <MoveIcon size={14} />
        </button>
        <button
          className="icon-btn"
          aria-label="Merge into another author"
          title={selectionActive ? 'Clear selection to merge this author' : 'Merge into another author'}
          disabled={selectionActive}
          onClick={(e) => {
            e.stopPropagation();
            onMergeAuthor(group.author);
          }}
        >
          <MergeIcon size={14} />
        </button>
      </div>

      {expanded && (
        <div className="book-list">
          {group.books.map((book) => (
            <BookRow
              key={book.bookId}
              book={book}
              selectedFileIds={selectedFileIds}
              selectedBookIds={selectedBookIds}
              onToggleSelect={onToggleSelect}
              onToggleSelectBook={onToggleSelectBook}
              onRenameBook={onRenameBook}
              onRenameFile={onRenameFile}
              onMoveBook={onMoveBook}
              onMergeBook={onMergeBook}
              onMatchBook={onMatchBook}
            />
          ))}
        </div>
      )}
    </div>
  );
}
