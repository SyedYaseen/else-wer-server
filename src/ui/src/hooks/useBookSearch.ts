import { useEffect, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import type { AudioBookRow } from '../types/book';
import { listBooks } from '../api/books';

// Debounced search: empty query returns the already-fetched `books` array
// (zero network cost, preserves the instant default view); non-empty query
// hits GET /list_books?q= server-side (src/db/audiobooks.rs::search_books)
// instead of filtering client-side.
export function useBookSearch(books: AudioBookRow[], delayMs = 200) {
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => setDebouncedQuery(query), delayMs);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [query, delayMs]);

  const trimmed = debouncedQuery.trim();
  const hasQuery = trimmed.length > 0;

  const { data: searchResults } = useQuery({
    queryKey: ['books', 'search', trimmed],
    queryFn: () => listBooks(trimmed),
    enabled: hasQuery,
    placeholderData: (prev) => prev,
  });

  const filtered = hasQuery ? (searchResults ?? []) : books;

  return {
    query,
    setQuery,
    debouncedQuery,
    filtered,
    hasQuery,
  };
}
