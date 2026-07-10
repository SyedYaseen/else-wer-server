import { api } from './client';
import type { AudioBookRow, Progress, FileMetadata } from '../types/book';

interface ListBooksResponse {
  message: string;
  count: number;
  books: AudioBookRow[];
}

interface FileMetadataResponse {
  message: string;
  count: number;
  data: FileMetadata[];
}

export async function listBooks(): Promise<AudioBookRow[]> {
  const data = await api.get<ListBooksResponse>('/list_books');
  return data.books;
}

export async function listInProgress(): Promise<Progress[]> {
  return api.get<Progress[]>('/list_inprogress');
}

export async function fileMetadata(bookId: number): Promise<FileMetadata[]> {
  const data = await api.get<FileMetadataResponse>(`/file_metadata/${bookId}`);
  return data.data;
}

export async function rescanFiles(): Promise<void> {
  await api.get('/scan_files');
}

// The server writes replacement cover art to the same path/filename as the old
// one (see book_cover.rs), so the URL doesn't change when a cover is replaced.
// Track a per-path cache-bust token so callers who just replaced a cover can
// force the browser to refetch instead of showing the stale cached image.
const coverBust = new Map<string, number>();

export function bumpCoverCache(coverArt: string | null | undefined): void {
  if (coverArt) coverBust.set(coverArt, Date.now());
}

export function coverUrl(coverArt: string | null): string | null {
  if (!coverArt) return null;
  const bust = coverBust.get(coverArt);
  return `/api${coverArt}${bust ? `?v=${bust}` : ''}`;
}
