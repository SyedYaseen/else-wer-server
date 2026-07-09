import { useEffect, useRef, useState } from 'react';
import type { AudioBookRow } from '../types/book';

// Ported from else-wer-app/components/hooks/useBooksSearch.ts (debounced client-side filter).
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

  let filtered = books;
  if (debouncedQuery.trim().length > 0) {
    const q = debouncedQuery.toLowerCase();
    filtered = books.filter(
      (book) =>
        book.title?.toLowerCase().includes(q) ||
        book.author?.toLowerCase().includes(q) ||
        book.series?.toLowerCase().includes(q) ||
        book.narrated_by?.toLowerCase().includes(q),
    );
  }

  return {
    query,
    setQuery,
    debouncedQuery,
    filtered,
    hasQuery: debouncedQuery.trim().length > 0,
  };
}
