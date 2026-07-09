import { useState } from 'react';
import type { AudioBookRow } from '../../types/book';
import { coverUrl } from '../../api/books';
import { BookCard } from './BookCard';
import { ChevronRightIcon } from '../organize/icons';
import './library.css';

const COLLAGE_SIZE = 4;

interface BookGroupSectionProps {
  label: string;
  books: AudioBookRow[];
  selectable?: boolean;
  selectedIds?: Set<number>;
  onToggleSelect?: (id: number) => void;
}

export function BookGroupSection({
  label,
  books,
  selectable,
  selectedIds,
  onToggleSelect,
}: BookGroupSectionProps) {
  const [expanded, setExpanded] = useState(false);
  const collage = books.slice(0, COLLAGE_SIZE);

  return (
    <div className="book-group">
      <button
        type="button"
        className="book-group-header"
        onClick={() => setExpanded((e) => !e)}
        aria-expanded={expanded}
      >
        <ChevronRightIcon className={`book-group-chevron ${expanded ? 'expanded' : ''}`} />
        <div className="book-group-info">
          <span className="book-group-label">{label}</span>
          <span className="book-group-count">
            {books.length} {books.length === 1 ? 'book' : 'books'}
          </span>
        </div>
        <div className="book-group-collage">
          {collage.map((book) => {
            const cover = coverUrl(book.cover_art);
            return cover ? (
              <img key={book.id} className="book-group-collage-cover" src={cover} alt="" loading="lazy" />
            ) : (
              <div key={book.id} className="book-group-collage-cover book-group-collage-placeholder">
                {book.title?.charAt(0).toUpperCase() ?? '?'}
              </div>
            );
          })}
        </div>
      </button>

      {expanded && (
        <div className="library-grid book-group-grid">
          {books.map((book) => (
            <BookCard
              key={book.id}
              book={book}
              selectable={selectable}
              selected={selectedIds?.has(book.id)}
              onToggleSelect={onToggleSelect}
            />
          ))}
        </div>
      )}
    </div>
  );
}
