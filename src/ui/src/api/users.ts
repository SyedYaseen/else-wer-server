import { api } from './client';

export interface UserSummary {
  id: number;
  username: string;
  is_admin: boolean;
  can_organize: boolean;
}

interface ListUsersResponse {
  users: UserSummary[];
}

export interface CreateUserPayload {
  username: string;
  password: string;
  is_admin: boolean;
  can_organize: boolean;
}

export interface UpdateUserPermissionsPayload {
  user_id: number;
  is_admin: boolean;
  can_organize: boolean;
}

export async function listUsers(): Promise<UserSummary[]> {
  const data = await api.get<ListUsersResponse>('/list_users');
  return data.users;
}

export async function createUser(payload: CreateUserPayload): Promise<void> {
  await api.post('/create_user', payload);
}

export async function deleteUser(userId: number): Promise<void> {
  await api.post('/delete_user', { user_id: userId });
}

export async function updateUserPermissions(payload: UpdateUserPermissionsPayload): Promise<void> {
  await api.post('/update_user_permissions', payload);
}
