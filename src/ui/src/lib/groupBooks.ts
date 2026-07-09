import type { AudioBookRow } from '../types/book';

export interface BookGroup {
  key: string;
  label: string;
  books: AudioBookRow[];
}

function seriesSequenceValue(book: AudioBookRow): number {
  const n = book.series_sequence ? parseFloat(book.series_sequence) : NaN;
  return Number.isNaN(n) ? Number.POSITIVE_INFINITY : n;
}

export function groupByAuthor(books: AudioBookRow[]): BookGroup[] {
  const groups = new Map<string, BookGroup>();

  for (const book of books) {
    const label = book.author?.trim() || 'Unknown author';
    const key = label.toLowerCase();
    let group = groups.get(key);
    if (!group) {
      group = { key, label, books: [] };
      groups.set(key, group);
    }
    group.books.push(book);
  }

  for (const group of groups.values()) {
    group.books.sort(
      (a, b) => seriesSequenceValue(a) - seriesSequenceValue(b) || a.title.localeCompare(b.title),
    );
  }

  return Array.from(groups.values()).sort((a, b) => a.label.localeCompare(b.label));
}

export interface SeriesOption {
  id: number;
  name: string;
  count: number;
}

// Pickable series for the Set-series sheet, derived from the books list (a series
// only exists with at least one linked book, so no separate endpoint is needed).
export function listSeries(books: AudioBookRow[]): SeriesOption[] {
  const series = new Map<number, SeriesOption>();
  for (const book of books) {
    if (book.series_id == null) continue;
    const existing = series.get(book.series_id);
    if (existing) {
      existing.count += 1;
    } else {
      series.set(book.series_id, {
        id: book.series_id,
        name: book.series_name?.trim() || 'Unknown series',
        count: 1,
      });
    }
  }
  return Array.from(series.values()).sort((a, b) => a.name.localeCompare(b.name));
}

// Strictly real-world series (series_id): the plain-text `series` column mirrors the
// album/title stop-gap, so falling back to it would show every book as its own group.
export function groupBySeries(books: AudioBookRow[]): BookGroup[] {
  const groups = new Map<string, BookGroup>();

  for (const book of books) {
    if (book.series_id == null) continue;
    const label = book.series_name?.trim() || 'Unknown series';
    const key = `id:${book.series_id}`;
    let group = groups.get(key);
    if (!group) {
      group = { key, label, books: [] };
      groups.set(key, group);
    }
    group.books.push(book);
  }

  for (const group of groups.values()) {
    group.books.sort(
      (a, b) => seriesSequenceValue(a) - seriesSequenceValue(b) || a.title.localeCompare(b.title),
    );
  }

  return Array.from(groups.values()).sort((a, b) => a.label.localeCompare(b.label));
}
