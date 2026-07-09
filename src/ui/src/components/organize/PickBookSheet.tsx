import { useMemo, useState } from 'react';
import { BottomSheet } from '../ui/BottomSheet';
import { Button } from '../ui/Button';
import type { AuthorGroup } from '../../types/scan';

export type PickResult =
  | { kind: 'existing'; bookId: number; author: string; series: string }
  | { kind: 'new'; author: string; title: string };

interface PickBookSheetProps {
  open: boolean;
  onClose: () => void;
  title: string;
  tree: AuthorGroup[];
  excludeBookId?: number;
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
  const [newAuthor, setNewAuthor] = useState('');
  const [newTitle, setNewTitle] = useState('');

  const books = useMemo(() => {
    const q = query.trim().toLowerCase();
    const rows: { bookId: number; author: string; series: string; title: string }[] = [];
    for (const a of tree) {
      for (const b of a.books) {
        if (b.bookId === excludeBookId) continue;
        if (q && !b.title.toLowerCase().includes(q) && !a.author.toLowerCase().includes(q)) continue;
        rows.push({ bookId: b.bookId, author: a.author, series: b.series, title: b.title });
      }
    }
    return rows;
  }, [tree, query, excludeBookId]);

  if (!open) return null;

  return (
    <BottomSheet open={open} onClose={onClose}>
      <h3 className="sheet-title">{title}</h3>

      {allowCreateNew && (
        <div className="pick-create-new">
          <div className="pick-create-new-label">Create a new book</div>
          <div className="pick-create-new-row">
            <input
              className="organize-input"
              placeholder="Author"
              value={newAuthor}
              onChange={(e) => setNewAuthor(e.target.value)}
            />
            <input
              className="organize-input"
              placeholder="Title"
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
            />
            <Button
              variant="secondary"
              disabled={!newAuthor.trim() || !newTitle.trim()}
              onClick={() => {
                onPick({ kind: 'new', author: newAuthor.trim(), title: newTitle.trim() });
                onClose();
              }}
            >
              Create
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
