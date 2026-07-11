import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { listScannedFiles, saveOrganizedFiles, backfillMetadata, assignSeries } from '../api/scan';
import { rescanFiles } from '../api/books';
import { LIBRARY_KEYS } from './useLibraryBooks';
import type { ChangeDto } from '../types/scan';

export const SCAN_KEYS = {
  files: ['scannedFiles'] as const,
};

// libraryId omitted or undefined -> all libraries (today's default behavior).
export function useScannedFiles(libraryId?: number) {
  return useQuery({
    queryKey: libraryId === undefined ? SCAN_KEYS.files : [...SCAN_KEYS.files, libraryId],
    queryFn: () => listScannedFiles(libraryId),
  });
}

export function useApplyChanges() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (changes: ChangeDto[]) => saveOrganizedFiles(changes),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: SCAN_KEYS.files });
      queryClient.invalidateQueries({ queryKey: LIBRARY_KEYS.books });
    },
  });
}

export function useRescanScannedFiles() {
  const queryClient = useQueryClient();
  return async () => {
    await rescanFiles();
    await queryClient.invalidateQueries({ queryKey: SCAN_KEYS.files });
  };
}

export function useBackfillMetadata() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: backfillMetadata,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: SCAN_KEYS.files });
      queryClient.invalidateQueries({ queryKey: LIBRARY_KEYS.books });
    },
  });
}

export function useAssignSeries() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: assignSeries,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: SCAN_KEYS.files });
      queryClient.invalidateQueries({ queryKey: LIBRARY_KEYS.books });
    },
  });
}
