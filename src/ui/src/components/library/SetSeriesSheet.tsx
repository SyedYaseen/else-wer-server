import { useEffect, useMemo, useState } from 'react';
import { BottomSheet } from '../ui/BottomSheet';
import { Button } from '../ui/Button';
import { useAssignSeries } from '../../hooks/useOrganize';
import { useLibraryBooks } from '../../hooks/useLibraryBooks';
import { listSeries } from '../../lib/groupBooks';
import './library.css';

export interface SetSeriesBook {
  id: number;
  title: string;
  sequence: string | null;
}

type Selection =
  | { kind: 'existing'; id: number; name: string }
  | { kind: 'new'; name: string }
  | { kind: 'clear' };

interface SetSeriesSheetProps {
  open: boolean;
  onClose: () => void;
  books: SetSeriesBook[];
  onSuccess?: () => void;
}

export function SetSeriesSheet({ open, onClose, books, onSuccess }: SetSeriesSheetProps) {
  const [query, setQuery] = useState('');
  const [selection, setSelection] = useState<Selection | null>(null);
  const [sequences, setSequences] = useState<Record<number, string>>({});
  const { data: allBooks } = useLibraryBooks();
  const { mutate, isPending, isError, reset } = useAssignSeries();

  useEffect(() => {
    if (open) {
      setQuery('');
      setSelection(null);
      setSequences(Object.fromEntries(books.map((b) => [b.id, b.sequence ?? ''])));
      reset();
    }
  }, [open, books, reset]);

  const options = useMemo(() => listSeries(allBooks ?? []), [allBooks]);

  if (!open) return null;

  const q = query.trim();
  const filtered = q
    ? options.filter((s) => s.name.toLowerCase().includes(q.toLowerCase()))
    : options;
  const exactMatch = options.some((s) => s.name.toLowerCase() === q.toLowerCase());

  function apply() {
    if (!selection) return;
    mutate(
      {
        series_id: selection.kind === 'existing' ? selection.id : undefined,
        series_name: selection.kind === 'new' ? selection.name : undefined,
        books: books.map((b) => ({
          book_id: b.id,
          sequence: selection.kind === 'clear' ? null : sequences[b.id]?.trim() || null,
        })),
      },
      {
        onSuccess: () => {
          onClose();
          onSuccess?.();
        },
      },
    );
  }

  return (
    <BottomSheet open={open} onClose={onClose}>
      <h3 className="sheet-title">
        Set series for {books.length} book{books.length === 1 ? '' : 's'}
      </h3>

      <input
        className="organize-input pick-search"
        placeholder="Search or type a new series name…"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        autoFocus
      />

      <div className="pick-list set-series-list">
        {filtered.map((s) => (
          <button
            key={s.id}
            className={`pick-item${
              selection?.kind === 'existing' && selection.id === s.id ? ' set-series-selected' : ''
            }`}
            onClick={() => setSelection({ kind: 'existing', id: s.id, name: s.name })}
          >
            <span className="pick-item-title">{s.name}</span>
            <span className="pick-item-author">
              {s.count} book{s.count === 1 ? '' : 's'}
            </span>
          </button>
        ))}
        {q && !exactMatch && (
          <button
            className={`pick-item${selection?.kind === 'new' ? ' set-series-selected' : ''}`}
            onClick={() => setSelection({ kind: 'new', name: q })}
          >
            <span className="pick-item-title">Create series “{q}”</span>
          </button>
        )}
        <button
          className={`pick-item${selection?.kind === 'clear' ? ' set-series-selected' : ''}`}
          onClick={() => setSelection({ kind: 'clear' })}
        >
          <span className="pick-item-title set-series-clear">Remove from series</span>
        </button>
      </div>

      {selection && selection.kind !== 'clear' && (
        <div className="set-series-books">
          {books.map((b) => (
            <div key={b.id} className="set-series-book-row">
              <span className="set-series-book-title">{b.title}</span>
              <input
                className="organize-input set-series-seq-input"
                inputMode="decimal"
                placeholder="#"
                value={sequences[b.id] ?? ''}
                onChange={(e) => setSequences((prev) => ({ ...prev, [b.id]: e.target.value }))}
              />
            </div>
          ))}
        </div>
      )}

      {isError && <div className="set-series-error">Could not update series. Try again.</div>}

      <Button onClick={apply} disabled={!selection || isPending}>
        {isPending ? 'Applying…' : 'Apply'}
      </Button>
    </BottomSheet>
  );
}
