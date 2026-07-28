import { api } from './client';
import { useAuthStore } from '../store/auth';
import type { Progress } from '../types/book';

export interface ProgressUpdate {
  book_id: number;
  file_id: number;
  progress_ms: number;
  complete: boolean;
  listened_delta_ms?: number;
  // Client wall-clock save time (ISO). The server's upsert uses it as a
  // last-write-wins guard so a replayed offline save can't overwrite newer
  // progress another device wrote in the meantime.
  updated_at?: string;
}

export async function getBookProgress(bookId: number): Promise<Progress[]> {
  return api.get<Progress[]>(`/get_book_progress/${bookId}`);
}

export async function updateProgress(payload: ProgressUpdate): Promise<void> {
  await api.post('/update_progress', payload);
}

// <audio src> can't set an Authorization header, so /api/stream/{id} falls back
// to a `?token=` query param (StreamAuth extractor, server-side).
export function streamUrl(fileId: number): string {
  const token = useAuthStore.getState().token;
  return `/api/stream/${fileId}${token ? `?token=${encodeURIComponent(token)}` : ''}`;
}
