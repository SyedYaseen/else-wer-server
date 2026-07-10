import { create } from 'zustand';
import { downloadBook } from './storage';
import type { AudioBookRow, FileMetadata } from '../types/book';

export type DownloadEntry =
  | { status: 'downloading'; fraction: number }
  | { status: 'done' }
  | { status: 'error'; message: string };

interface DownloadStore {
  byBook: Record<number, DownloadEntry>;
}

export const useDownloadStore = create<DownloadStore>(() => ({ byBook: {} }));

function setEntry(bookId: number, entry: DownloadEntry) {
  useDownloadStore.setState((s) => ({ byBook: { ...s.byBook, [bookId]: entry } }));
}

// Module-level so unmounting the route (navigating to the player, iOS Safari
// re-rendering the standalone PWA) neither loses the progress UI nor lets a
// second tap start a duplicate concurrent download of the same book.
export async function startDownload(book: AudioBookRow, files: FileMetadata[]): Promise<void> {
  const existing = useDownloadStore.getState().byBook[book.id];
  if (existing?.status === 'downloading') return;
  setEntry(book.id, { status: 'downloading', fraction: 0 });
  let lastShown = 0;
  try {
    await downloadBook(book, files, (fraction) => {
      // Throttle to 1% steps — the raw callback fires per network chunk,
      // which re-renders subscribers hundreds of times a second otherwise.
      if (fraction - lastShown >= 0.01 || fraction >= 1) {
        lastShown = fraction;
        setEntry(book.id, { status: 'downloading', fraction });
      }
    });
    setEntry(book.id, { status: 'done' });
  } catch (e) {
    setEntry(book.id, { status: 'error', message: e instanceof Error ? e.message : 'Download failed' });
  }
}
