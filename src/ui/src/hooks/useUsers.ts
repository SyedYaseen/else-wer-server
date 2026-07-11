import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  listUsers,
  createUser,
  deleteUser,
  updateUserPermissions,
  type CreateUserPayload,
  type UpdateUserPermissionsPayload,
} from '../api/users';

export const USER_KEYS = {
  users: ['users'] as const,
};

export function useUsers() {
  return useQuery({ queryKey: USER_KEYS.users, queryFn: listUsers });
}

export function useCreateUser() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: CreateUserPayload) => createUser(payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: USER_KEYS.users });
    },
  });
}

export function useDeleteUser() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (userId: number) => deleteUser(userId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: USER_KEYS.users });
    },
  });
}

export function useUpdateUserPermissions() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: UpdateUserPermissionsPayload) => updateUserPermissions(payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: USER_KEYS.users });
    },
  });
}
