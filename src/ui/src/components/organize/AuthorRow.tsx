import { useState } from 'react';
import type { AuthorGroup, BookGroup, FileInfo } from '../../types/scan';
import { BookRow } from './BookRow';
import { ChevronRightIcon, PencilIcon } from './icons';

interface AuthorRowProps {
  group: AuthorGroup;
  selectedFileIds: Set<number>;
  selectedBookIds: Set<number>;
  onToggleSelect: (id: number) => void;
  onToggleSelectBook: (bookId: number) => void;
  onRenameAuthor: (author: string) => void;
  onRenameBook: (book: BookGroup) => void;
  onRenameFile: (file: FileInfo) => void;
  onMergeBook: (book: BookGroup) => void;
  onMatchBook: (book: BookGroup) => void;
}

export function AuthorRow({
  group,
  selectedFileIds,
  selectedBookIds,
  onToggleSelect,
  onToggleSelectBook,
  onRenameAuthor,
  onRenameBook,
  onRenameFile,
  onMergeBook,
  onMatchBook,
}: AuthorRowProps) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="author-row">
      <div className="author-header" onClick={() => setExpanded((e) => !e)}>
        <ChevronRightIcon className={`author-chevron ${expanded ? 'expanded' : ''}`} />
        <span className="author-name">{group.author}</span>
        <span className="author-book-count">
          {group.books.length} {group.books.length === 1 ? 'book' : 'books'}
        </span>
        <button
          className="icon-btn"
          aria-label="Rename author"
          onClick={(e) => {
            e.stopPropagation();
            onRenameAuthor(group.author);
          }}
        >
          <PencilIcon size={14} />
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
              onMergeBook={onMergeBook}
              onMatchBook={onMatchBook}
            />
          ))}
        </div>
      )}
    </div>
  );
}
