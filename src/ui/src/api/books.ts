import { api } from './client';
import { useAuthStore } from '../store/auth';
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

export function coverUrl(coverArt: string | null): string | null {
  return coverArt ? `/api${coverArt}` : null;
}

// /api/download_book/{id} builds a whole-book zip in memory on the server, which
// the server can't afford — download each file instead via /api/stream/{id},
// the same Range-capable endpoint used for playback, which streams from disk
// without buffering. Files are saved individually via blob + object URL, same
// mechanic as before, one per file in the book.
export async function downloadBook(bookId: number): Promise<void> {
  const token = useAuthStore.getState().token;
  const headers: HeadersInit = token ? { Authorization: `Bearer ${token}` } : {};
  const files = (await fileMetadata(bookId)).slice().sort(
    (a, b) => (a.track_number ?? 0) - (b.track_number ?? 0),
  );
  for (const file of files) {
    const res = await fetch(`/api/stream/${file.id}`, { headers });
    if (!res.ok) throw new Error(`Download failed: ${res.status}`);
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = file.file_name;
    a.click();
    URL.revokeObjectURL(url);
  }
}
