import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { BottomSheet } from '../ui/BottomSheet';
import { Button } from '../ui/Button';
import { Pill } from '../ui/Pill';
import { matchBookCandidates, applyBookMatch } from '../../api/scan';
import { SCAN_KEYS } from '../../hooks/useOrganize';
import { LIBRARY_KEYS } from '../../hooks/useLibraryBooks';
import { SearchIcon } from './icons';
import type { MatchCandidate } from '../../types/scan';

const SEARCH_DEBOUNCE_MS = 400;

interface MatchSheetProps {
  open: boolean;
  onClose: () => void;
  bookId: number | null;
}

export function MatchSheet({ open, onClose, bookId }: MatchSheetProps) {
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [applyTitleAuthor, setApplyTitleAuthor] = useState(true);
  const [applied, setApplied] = useState<MatchCandidate | null>(null);
  const queryClient = useQueryClient();

  // Reset per-open state so a stale confirmation/query doesn't leak to the next book.
  useEffect(() => {
    if (open) {
      setQuery('');
      setDebouncedQuery('');
      setApplied(null);
    }
  }, [open, bookId]);

  useEffect(() => {
    const t = setTimeout(() => setDebouncedQuery(query), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [query]);

  const { data, isLoading, isError } = useQuery({
    queryKey: ['matchBook', bookId, debouncedQuery],
    queryFn: () => matchBookCandidates(bookId!, debouncedQuery || undefined),
    enabled: open && bookId != null,
  });

  const apply = useMutation({
    mutationFn: (candidate: MatchCandidate) =>
      applyBookMatch(bookId!, { ...candidate, apply_title_author: applyTitleAuthor }),
    onSuccess: (_data, candidate) => {
      queryClient.invalidateQueries({ queryKey: SCAN_KEYS.files });
      queryClient.invalidateQueries({ queryKey: LIBRARY_KEYS.books });
      setApplied(candidate);
    },
  });

  if (!open || bookId == null) return null;

  if (applied) {
    return (
      <BottomSheet open={open} onClose={onClose}>
        <h3 className="sheet-title">Match metadata</h3>
        <div className="match-applied">
          <p>
            Applied — <strong>{applied.title}</strong>
            {applied.author ? ` by ${applied.author}` : ''}
          </p>
          {applyTitleAuthor && (
            <p className="match-applied-note">
              If the author changed, this book may now be listed under a different author group.
            </p>
          )}
        </div>
        <Button variant="primary" onClick={onClose}>
          Done
        </Button>
      </BottomSheet>
    );
  }

  return (
    <BottomSheet open={open} onClose={onClose}>
      <h3 className="sheet-title">Match metadata</h3>
      {data && (
        <p className="match-current">
          Currently: <strong>{data.book_title}</strong> — {data.book_author}
        </p>
      )}

      <form
        className="match-search-row"
        onSubmit={(e) => {
          e.preventDefault();
          setDebouncedQuery(query);
        }}
      >
        <input
          className="organize-input"
          placeholder="Override search (optional)"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <button type="submit" className="icon-btn" aria-label="Search">
          <SearchIcon />
        </button>
      </form>

      <label className="apply-title-toggle">
        <input
          type="checkbox"
          checked={applyTitleAuthor}
          onChange={(e) => setApplyTitleAuthor(e.target.checked)}
        />
        Overwrite title/author with match
      </label>

      {isLoading && <div className="match-state">Searching…</div>}
      {isError && <div className="match-state">Search failed.</div>}

      <div className="match-list">
        {data?.candidates.map((c, i) => (
          <div key={i} className="match-candidate">
            {c.cover_url && <img className="match-cover" src={c.cover_url} alt="" />}
            <div className="match-candidate-body">
              <div className="match-candidate-title">{c.title}</div>
              <div className="match-candidate-author">{c.author}</div>
              {c.series_name && (
                <div className="match-candidate-series">
                  {c.series_name}
                  {c.series_sequence ? ` #${c.series_sequence}` : ''}
                </div>
              )}
            </div>
            <div className="match-candidate-actions">
              <Pill tone={c.confidence > 0.8 ? 'sage' : c.confidence > 0.5 ? 'warning' : 'danger'}>
                {Math.round(c.confidence * 100)}%
              </Pill>
              <Button variant="secondary" disabled={apply.isPending} onClick={() => apply.mutate(c)}>
                Apply
              </Button>
            </div>
          </div>
        ))}
        {data && data.candidates.length === 0 && <div className="match-state">No candidates found.</div>}
      </div>
    </BottomSheet>
  );
}
