import { useQuery, useQueryClient } from '@tanstack/react-query';
import { listBooks, listInProgress, rescanFiles } from '../api/books';

export const LIBRARY_KEYS = {
  books: ['books'] as const,
  inProgress: ['inprogress'] as const,
};

export function useLibraryBooks() {
  return useQuery({ queryKey: LIBRARY_KEYS.books, queryFn: listBooks });
}

export function useInProgressBooks() {
  return useQuery({ queryKey: LIBRARY_KEYS.inProgress, queryFn: listInProgress });
}

export function useRescan() {
  const queryClient = useQueryClient();
  return async () => {
    await rescanFiles();
    await queryClient.invalidateQueries({ queryKey: LIBRARY_KEYS.books });
  };
}
