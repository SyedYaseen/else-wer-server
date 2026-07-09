import { Link } from 'react-router-dom';
import type { AudioBookRow } from '../../types/book';
import { coverUrl } from '../../api/books';
import './library.css';

interface BookCardProps {
  book: AudioBookRow;
  selectable?: boolean;
  selected?: boolean;
  onToggleSelect?: (id: number) => void;
}

export function BookCard({ book, selectable, selected, onToggleSelect }: BookCardProps) {
  const cover = coverUrl(book.cover_art);

  const body = (
    <>
      {cover ? (
        <img className="book-card-cover" src={cover} alt="" loading="lazy" />
      ) : (
        <div className="book-card-cover-placeholder">{book.title?.charAt(0).toUpperCase() ?? '?'}</div>
      )}
      <div className="book-card-details">
        <div className="book-card-title">{book.title}</div>
        <div className="book-card-author">{book.author}</div>
        {book.series && <div className="book-card-meta">{book.series}</div>}
        {book.narrated_by && <div className="book-card-meta">Narrated by {book.narrated_by}</div>}
      </div>
    </>
  );

  if (selectable) {
    return (
      <button
        type="button"
        className={`book-card book-card-selectable${selected ? ' book-card-selected' : ''}`}
        onClick={() => onToggleSelect?.(book.id)}
        aria-pressed={selected}
      >
        <span className="book-card-check" aria-hidden>
          {selected ? '✓' : ''}
        </span>
        {body}
      </button>
    );
  }

  return (
    <Link to={`/book/${book.id}`} className="book-card">
      {body}
    </Link>
  );
}
