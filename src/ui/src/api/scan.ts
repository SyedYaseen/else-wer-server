import { api } from './client';
import type { GroupedFiles, ChangeDto, MatchResponse, ApplyMatchDto } from '../types/scan';

interface ListScannedFilesResponse {
  files: GroupedFiles;
}

export async function listScannedFiles(): Promise<GroupedFiles> {
  const data = await api.get<ListScannedFilesResponse>('/list_scanned_files');
  return data.files;
}

export async function saveOrganizedFiles(changes: ChangeDto[]): Promise<void> {
  await api.post('/save_organized_files', changes);
}

export async function matchBookCandidates(bookId: number, q?: string): Promise<MatchResponse> {
  const query = q ? `?q=${encodeURIComponent(q)}` : '';
  return api.get<MatchResponse>(`/match_book/${bookId}${query}`);
}

export async function applyBookMatch(bookId: number, payload: ApplyMatchDto): Promise<void> {
  await api.post(`/match_book/${bookId}`, payload);
}

export interface BackfillMetadataResult {
  checked: number;
  updated: number;
  skipped: number;
}

export async function backfillMetadata(): Promise<BackfillMetadataResult> {
  return api.post<BackfillMetadataResult>('/backfill_metadata');
}

// Mirrors src/models/match_meta.rs::AssignSeriesDto (POST /api/assign_series)
export interface AssignSeriesPayload {
  series_id?: number | null;
  series_name?: string | null;
  books: { book_id: number; sequence: string | null }[];
}

export interface AssignSeriesResult {
  series_id: number | null;
  updated: number;
}

export async function assignSeries(payload: AssignSeriesPayload): Promise<AssignSeriesResult> {
  return api.post<AssignSeriesResult>('/assign_series', payload);
}
