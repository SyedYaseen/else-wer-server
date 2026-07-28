import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  createLibrary,
  updateLibrary,
  deleteLibrary,
  scanLibrary,
  type CreateLibraryPayload,
  type UpdateLibraryPayload,
} from '../api/libraries';
import { LIBRARY_KEYS } from './useLibraryBooks';

export function useCreateLibrary() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: CreateLibraryPayload) => createLibrary(payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: LIBRARY_KEYS.libraries });
    },
  });
}

export function useUpdateLibrary() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, payload }: { id: number; payload: UpdateLibraryPayload }) =>
      updateLibrary(id, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: LIBRARY_KEYS.libraries });
    },
  });
}

export function useDeleteLibrary() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteLibrary(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: LIBRARY_KEYS.libraries });
      queryClient.invalidateQueries({ queryKey: LIBRARY_KEYS.books });
    },
  });
}

export function useScanLibrary() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => scanLibrary(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: LIBRARY_KEYS.books });
    },
  });
}
