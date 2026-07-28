import { useMemo, useState } from 'react';
import { BottomSheet } from '../ui/BottomSheet';
import { Button } from '../ui/Button';
import { SearchableCombobox, matchesQuery } from './SearchableCombobox';
import type { AuthorGroup, BookGroup } from '../../types/scan';

export type PickResult =
  | { kind: 'existing'; bookId: number; author: string; series: string }
  | { kind: 'new'; author: string; title: string };

interface PickBookSheetProps {
  open: boolean;
  onClose: () => void;
  title: string;
  tree: AuthorGroup[];
  excludeBookId?: number | number[];
  allowCreateNew?: boolean;
  onPick: (target: PickResult) => void;
}

export function PickBookSheet({
  open,
  onClose,
  title,
  tree,
  excludeBookId,
  allowCreateNew,
  onPick,
}: PickBookSheetProps) {
  const [query, setQuery] = useState('');
  const [author, setAuthor] = useState('');
  const [isNewAuthor, setIsNewAuthor] = useState(false);
  const [bookTitle, setBookTitle] = useState('');

  const books = useMemo(() => {
    const excluded = new Set(
      excludeBookId === undefined
        ? []
        : Array.isArray(excludeBookId)
          ? excludeBookId
          : [excludeBookId],
    );
    const rows: { bookId: number; author: string; series: string; title: string }[] = [];
    for (const a of tree) {
      for (const b of a.books) {
        if (excluded.has(b.bookId)) continue;
        if (!matchesQuery(b.title, query) && !matchesQuery(a.author, query)) continue;
        rows.push({ bookId: b.bookId, author: a.author, series: b.series, title: b.title });
      }
    }
    return rows;
  }, [tree, query, excludeBookId]);

  // Once an existing author is picked, the book combobox only offers that
  // author's existing books - typing something new still creates a new book.
  const authorBooks: BookGroup[] = useMemo(() => {
    if (isNewAuthor || !author) return [];
    return tree.find((a) => a.author === author)?.books ?? [];
  }, [tree, author, isNewAuthor]);

  function reset() {
    setQuery('');
    setAuthor('');
    setIsNewAuthor(false);
    setBookTitle('');
  }

  if (!open) return null;

  return (
    <BottomSheet
      open={open}
      onClose={() => {
        reset();
        onClose();
      }}
    >
      <h3 className="sheet-title">{title}</h3>

      {allowCreateNew && (
        <div className="pick-create-new">
          <div className="pick-create-new-label">Create or pick a destination book</div>
          <div className="rename-form">
            <label className="organize-field">
              <span>Author</span>
              <SearchableCombobox
                items={tree.map((a) => a.author)}
                getLabel={(a) => a}
                value={author}
                placeholder="Author"
                onSelectExisting={(a) => {
                  setAuthor(a);
                  setIsNewAuthor(false);
                  setBookTitle('');
                }}
                onCreateNew={(text) => {
                  setAuthor(text);
                  setIsNewAuthor(true);
                  setBookTitle('');
                }}
              />
            </label>
            <label className="organize-field">
              <span>Book title</span>
              <SearchableCombobox
                items={authorBooks}
                getLabel={(b) => b.title}
                value={bookTitle}
                placeholder="Title"
                disabled={!author.trim()}
                onSelectExisting={(b) => {
                  onPick({ kind: 'existing', bookId: b.bookId, author: b.author, series: b.series });
                  reset();
                  onClose();
                }}
                onCreateNew={(text) => setBookTitle(text)}
              />
            </label>
            <Button
              variant="secondary"
              disabled={!author.trim() || !bookTitle.trim()}
              onClick={() => {
                onPick({ kind: 'new', author: author.trim(), title: bookTitle.trim() });
                reset();
                onClose();
              }}
            >
              Create "{bookTitle.trim() || '…'}"
            </Button>
          </div>
        </div>
      )}

      <input
        className="organize-input pick-search"
        placeholder="Search books…"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        autoFocus
      />

      <div className="pick-list">
        {books.map((b) => (
          <button
            key={b.bookId}
            className="pick-item"
            onClick={() => {
              onPick({ kind: 'existing', bookId: b.bookId, author: b.author, series: b.series });
              reset();
              onClose();
            }}
          >
            <span className="pick-item-title">{b.title}</span>
            <span className="pick-item-author">{b.author}</span>
          </button>
        ))}
        {books.length === 0 && <div className="pick-empty">No matching books</div>}
      </div>
    </BottomSheet>
  );
}
