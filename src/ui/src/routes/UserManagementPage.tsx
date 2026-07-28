import { useState, type FormEvent } from 'react';
import { Link } from 'react-router-dom';
import { Button } from '../components/ui/Button';
import { Card } from '../components/ui/Card';
import { Pill } from '../components/ui/Pill';
import { ActionMenu } from '../components/ui/ActionMenu';
import { BottomSheet } from '../components/ui/BottomSheet';
import { ChevronRightIcon } from '../components/organize/icons';
import { useAuthStore } from '../store/auth';
import { useUsers, useCreateUser, useDeleteUser, useUpdateUserPermissions } from '../hooks/useUsers';
import type { UserSummary } from '../api/users';
import './user-management.css';

type SheetState = { kind: 'add' } | { kind: 'delete'; user: UserSummary } | null;

export function UserManagementPage() {
  const { data: users = [], isLoading, isError, refetch } = useUsers();
  const createUser = useCreateUser();
  const deleteUser = useDeleteUser();
  const updatePermissions = useUpdateUserPermissions();
  const currentUsername = useAuthStore((s) => s.claims?.username);
  const [sheet, setSheet] = useState<SheetState>(null);

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [isAdmin, setIsAdmin] = useState(false);
  const [canOrganize, setCanOrganize] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const adminCount = users.filter((u) => u.is_admin).length;

  function closeAddSheet() {
    setSheet(null);
    setUsername('');
    setPassword('');
    setIsAdmin(false);
    setCanOrganize(false);
    setFormError(null);
  }

  async function handleCreate(e: FormEvent) {
    e.preventDefault();
    setFormError(null);
    try {
      await createUser.mutateAsync({
        username,
        password,
        is_admin: isAdmin,
        can_organize: canOrganize,
      });
      closeAddSheet();
    } catch {
      setFormError('Could not create user. The username may already be taken.');
    }
  }

  function toggleAdmin(user: UserSummary) {
    updatePermissions.mutate({
      user_id: user.id,
      is_admin: !user.is_admin,
      can_organize: user.can_organize,
    });
  }

  function toggleOrganize(user: UserSummary) {
    updatePermissions.mutate({
      user_id: user.id,
      is_admin: user.is_admin,
      can_organize: !user.can_organize,
    });
  }

  async function handleDelete() {
    if (sheet?.kind !== 'delete') return;
    await deleteUser.mutateAsync(sheet.user.id);
    setSheet(null);
  }

  return (
    <div className="user-mgmt-page">
      <div className="user-mgmt-header">
        <div>
          <h1 className="user-mgmt-title">Manage users</h1>
          <p className="user-mgmt-subtitle">
            Create accounts and control who can organize the library or delete books
          </p>
        </div>
        <div className="user-mgmt-actions">
          <Button variant="primary" onClick={() => setSheet({ kind: 'add' })}>
            Add user
          </Button>
          <Link className="user-mgmt-back-link" to="/">
            <ChevronRightIcon size={16} className="rotate-180" /> Back to library
          </Link>
        </div>
      </div>

      {isLoading && <div className="user-mgmt-state">Loading users…</div>}
      {isError && (
        <div className="user-mgmt-state">
          Couldn't load users.
          <Button variant="primary" onClick={() => refetch()}>
            Try again
          </Button>
        </div>
      )}

      <div className="user-mgmt-list">
        {users.map((user) => {
          const isSelf = user.username === currentUsername;
          const isLastAdmin = user.is_admin && adminCount <= 1;
          return (
            <Card key={user.id} className="user-mgmt-row">
              <div className="user-mgmt-row-info">
                <span className="user-mgmt-username">
                  {user.username}
                  {isSelf && <span className="user-mgmt-you"> (you)</span>}
                </span>
                <div className="user-mgmt-pills">
                  {user.is_admin && <Pill tone="accent">Admin</Pill>}
                  {!user.is_admin && user.can_organize && <Pill tone="sage">Can organize</Pill>}
                </div>
              </div>
              <ActionMenu
                items={[
                  {
                    label: user.is_admin ? 'Remove admin' : 'Make admin',
                    onClick: () => toggleAdmin(user),
                    disabled: isLastAdmin || updatePermissions.isPending,
                  },
                  !user.is_admin && {
                    label: user.can_organize ? 'Revoke organize access' : 'Grant organize access',
                    onClick: () => toggleOrganize(user),
                    disabled: updatePermissions.isPending,
                  },
                  {
                    label: 'Delete user',
                    onClick: () => setSheet({ kind: 'delete', user }),
                    disabled: isSelf || isLastAdmin,
                  },
                ]}
              />
            </Card>
          );
        })}
      </div>

      <BottomSheet open={sheet?.kind === 'add'} onClose={closeAddSheet}>
        <h3 className="sheet-title">Add user</h3>
        <form onSubmit={handleCreate} className="user-mgmt-form">
          <label className="user-mgmt-field-label">
            Username
            <input value={username} onChange={(e) => setUsername(e.target.value)} required autoFocus />
          </label>
          <label className="user-mgmt-field-label">
            Password
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
            />
          </label>
          <label className="user-mgmt-checkbox">
            <input type="checkbox" checked={isAdmin} onChange={(e) => setIsAdmin(e.target.checked)} />
            Admin
          </label>
          <label className="user-mgmt-checkbox">
            <input
              type="checkbox"
              checked={canOrganize}
              onChange={(e) => setCanOrganize(e.target.checked)}
              disabled={isAdmin}
            />
            Can organize
          </label>
          {formError && <p className="user-mgmt-field-error">{formError}</p>}
          <Button type="submit" disabled={createUser.isPending}>
            {createUser.isPending ? 'Creating…' : 'Create user'}
          </Button>
        </form>
      </BottomSheet>

      <BottomSheet open={sheet?.kind === 'delete'} onClose={() => setSheet(null)}>
        <h3 className="sheet-title">Delete user</h3>
        {sheet?.kind === 'delete' && (
          <>
            <p className="user-mgmt-confirm-text">
              Delete <strong>{sheet.user.username}</strong>? This can't be undone.
            </p>
            <div className="user-mgmt-confirm-actions">
              <Button variant="ghost" onClick={() => setSheet(null)}>
                Cancel
              </Button>
              <Button variant="destructive" onClick={handleDelete} disabled={deleteUser.isPending}>
                {deleteUser.isPending ? 'Deleting…' : 'Delete'}
              </Button>
            </div>
          </>
        )}
      </BottomSheet>
    </div>
  );
}
