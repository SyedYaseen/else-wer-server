import { api } from './client';

// Mirrors src/models/libraries.rs::Library (GET /api/libraries)
export interface Library {
  id: number;
  name: string;
  path: string;
  is_default: boolean;
  created_at: string;
}

export interface CreateLibraryPayload {
  name: string;
  path: string;
}

export interface UpdateLibraryPayload {
  name?: string;
  path?: string;
  is_default?: boolean;
}

export interface ScanLibraryResult {
  message: string;
  files_scanned: number;
}

export async function listLibraries(): Promise<Library[]> {
  return api.get<Library[]>('/libraries');
}

export async function createLibrary(payload: CreateLibraryPayload): Promise<Library> {
  return api.post<Library>('/libraries', payload);
}

export async function updateLibrary(id: number, payload: UpdateLibraryPayload): Promise<Library> {
  return api.put<Library>(`/libraries/${id}`, payload);
}

export async function deleteLibrary(id: number): Promise<void> {
  await api.delete(`/libraries/${id}`);
}

export async function scanLibrary(id: number): Promise<ScanLibraryResult> {
  return api.post<ScanLibraryResult>(`/libraries/${id}/scan`);
}
