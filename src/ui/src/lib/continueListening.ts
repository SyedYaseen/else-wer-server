import type { AudioBookRow, Progress } from '../types/book';

export interface ContinueListeningItem {
  book: AudioBookRow;
  progressMs: number;
  updatedAt: string;
}

// The server only exposes per-file progress rows (no book-level aggregate), so
// group by book_id here: a book is "in progress" if at least one of its files
// isn't complete yet, and its progress is the sum across files vs. total duration.
export function buildContinueListening(
  books: AudioBookRow[],
  progress: Progress[],
): ContinueListeningItem[] {
  const byBook = new Map<number, Progress[]>();
  for (const row of progress) {
    const rows = byBook.get(row.book_id) ?? [];
    rows.push(row);
    byBook.set(row.book_id, rows);
  }

  const items: ContinueListeningItem[] = [];
  for (const [bookId, rows] of byBook) {
    const book = books.find((b) => b.id === bookId);
    if (!book) continue;
    if (rows.every((r) => r.complete)) continue;

    const progressMs = rows.reduce((sum, r) => sum + r.progress_ms, 0);
    const updatedAt = rows.reduce((latest, r) => (r.updated_at > latest ? r.updated_at : latest), rows[0].updated_at);
    items.push({ book, progressMs, updatedAt });
  }

  items.sort((a, b) => (a.updatedAt < b.updatedAt ? 1 : -1));
  return items;
}
