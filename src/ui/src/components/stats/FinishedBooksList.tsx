import { Link } from 'react-router-dom';
import type { AudioBookRow } from '../../types/book';
import type { FinishedBook } from '../../api/stats';
import { coverUrl } from '../../api/books';

interface FinishedBooksListProps {
  books: AudioBookRow[];
  finished: FinishedBook[];
}

export function FinishedBooksList({ books, finished }: FinishedBooksListProps) {
  if (finished.length === 0) {
    return <div className="settings-state">No finished books yet</div>;
  }

  return (
    <div className="stats-finished-list">
      {finished.map((f) => {
        const book = books.find((b) => b.id === f.book_id);
        if (!book) return null;
        const cover = coverUrl(book.cover_art);
        return (
          <Link key={f.book_id} to={`/book/${book.id}`} className="stats-finished-row">
            {cover ? (
              <img className="stats-finished-cover" src={cover} alt="" />
            ) : (
              <div className="stats-finished-cover stats-finished-cover-placeholder" />
            )}
            <div className="stats-finished-info">
              <span className="stats-finished-title">{book.title}</span>
              <span className="stats-finished-meta">
                {book.author} · finished {new Date(f.finished_at).toLocaleDateString()}
                {f.times_finished > 1 ? ` · ${f.times_finished}× ` : ''}
              </span>
            </div>
          </Link>
        );
      })}
    </div>
  );
}
