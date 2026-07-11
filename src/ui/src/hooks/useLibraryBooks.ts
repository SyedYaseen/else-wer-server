import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { deleteBook, listBooks, listInProgress, rescanFiles } from '../api/books';
import { listLibraries } from '../api/libraries';

export const LIBRARY_KEYS = {
  books: ['books'] as const,
  inProgress: ['inprogress'] as const,
  libraries: ['libraries'] as const,
};

// libraryId omitted or undefined -> all libraries (today's default behavior).
export function useLibraryBooks(libraryId?: number) {
  return useQuery({
    queryKey: libraryId === undefined ? LIBRARY_KEYS.books : [...LIBRARY_KEYS.books, libraryId],
    queryFn: () => listBooks(undefined, libraryId),
  });
}

export function useLibraries() {
  return useQuery({ queryKey: LIBRARY_KEYS.libraries, queryFn: listLibraries });
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

export function useDeleteBook() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (bookId: number) => deleteBook(bookId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: LIBRARY_KEYS.books });
    },
  });
}
