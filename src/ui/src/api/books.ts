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

export function coverUrl(coverArt: string | null): string | null {
  return coverArt ? `/api${coverArt}` : null;
}
