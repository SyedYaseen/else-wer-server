import { api } from './client';
import type { Bookmark } from '../types/book';

export interface CreateBookmarkPayload {
  book_id: number;
  file_id: number;
  timestamp_ms: number;
  note?: string;
}

export async function listBookmarks(bookId: number): Promise<Bookmark[]> {
  return api.get<Bookmark[]>(`/bookmarks/book/${bookId}`);
}

export async function createBookmark(payload: CreateBookmarkPayload): Promise<Bookmark> {
  return api.post<Bookmark>('/bookmarks', payload);
}

export async function deleteBookmark(bookmarkId: number): Promise<void> {
  await api.delete(`/bookmarks/${bookmarkId}`);
}
