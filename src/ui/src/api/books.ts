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

export async function listBooks(q?: string, libraryId?: number): Promise<AudioBookRow[]> {
  const params = new URLSearchParams();
  if (q && q.trim().length > 0) params.set('q', q.trim());
  if (libraryId !== undefined) params.set('library_id', String(libraryId));
  const query = params.toString();
  const data = await api.get<ListBooksResponse>(`/list_books${query ? `?${query}` : ''}`);
  return data.books;
}

export async function listInProgress(): Promise<Progress[]> {
  return api.get<Progress[]>('/list_inprogress');
}

export async function fileMetadata(bookId: number): Promise<FileMetadata[]> {
  const data = await api.get<FileMetadataResponse>(`/file_metadata/${bookId}`);
  return data.data;
}

export async function reorderFiles(bookId: number, fileIds: number[]): Promise<FileMetadata[]> {
  const data = await api.post<FileMetadataResponse>(`/file_metadata/${bookId}/reorder`, {
    file_ids: fileIds,
  });
  return data.data;
}

export async function rescanFiles(): Promise<void> {
  await api.get('/scan_files');
}

export async function deleteBook(bookId: number): Promise<void> {
  await api.post('/delete_book', { book_id: bookId });
}

// cover_art already carries a `?v=<mtime>` cache-busting query param from the
// server (see book_cover.rs create_cover_link), so it changes whenever the
// underlying image actually does.
export function coverUrl(coverArt: string | null): string | null {
  return coverArt ? `/api${coverArt}` : null;
}
