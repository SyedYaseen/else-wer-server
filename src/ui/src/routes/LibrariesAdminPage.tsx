import { useState, type FormEvent } from 'react';
import { Link } from 'react-router-dom';
import { Button } from '../components/ui/Button';
import { Card } from '../components/ui/Card';
import { Pill } from '../components/ui/Pill';
import { ActionMenu } from '../components/ui/ActionMenu';
import { BottomSheet } from '../components/ui/BottomSheet';
import { ChevronRightIcon } from '../components/organize/icons';
import { RefreshIcon } from '../components/ui/icons';
import { useLibraries } from '../hooks/useLibraryBooks';
import {
  useCreateLibrary,
  useDeleteLibrary,
  useScanLibrary,
  useUpdateLibrary,
} from '../hooks/useLibrariesAdmin';
import type { Library } from '../api/libraries';
import { showToast } from '../lib/toast';
import './user-management.css';

type SheetState = { kind: 'add' } | { kind: 'delete'; library: Library } | null;

export function LibrariesAdminPage() {
  const { data: libraries = [], isLoading, isError, refetch } = useLibraries();
  const createLibrary = useCreateLibrary();
  const deleteLibrary = useDeleteLibrary();
  const scanLibrary = useScanLibrary();
  const updateLibrary = useUpdateLibrary();
  const [sheet, setSheet] = useState<SheetState>(null);
  const [scanningId, setScanningId] = useState<number | null>(null);

  const [name, setName] = useState('');
  const [path, setPath] = useState('');
  const [formError, setFormError] = useState<string | null>(null);

  function closeAddSheet() {
    setSheet(null);
    setName('');
    setPath('');
    setFormError(null);
  }

  async function handleCreate(e: FormEvent) {
    e.preventDefault();
    setFormError(null);
    try {
      await createLibrary.mutateAsync({ name, path });
      closeAddSheet();
    } catch {
      setFormError('Could not create library. The name or path may already be in use.');
    }
  }

  async function handleDelete() {
    if (sheet?.kind !== 'delete') return;
    await deleteLibrary.mutateAsync(sheet.library.id);
    setSheet(null);
  }

  async function handleSetDefault(library: Library) {
    try {
      await updateLibrary.mutateAsync({ id: library.id, payload: { is_default: true } });
    } catch {
      showToast(`Couldn't set ${library.name} as default`, 'error');
    }
  }

  async function handleScan(library: Library) {
    setScanningId(library.id);
    try {
      const result = await scanLibrary.mutateAsync(library.id);
      showToast(`${library.name}: ${result.files_scanned} file(s) scanned`, 'success');
    } catch {
      showToast(`Couldn't scan ${library.name}`, 'error');
    } finally {
      setScanningId(null);
    }
  }

  return (
    <div className="user-mgmt-page">
      <div className="user-mgmt-header">
        <div>
          <h1 className="user-mgmt-title">Manage libraries</h1>
          <p className="user-mgmt-subtitle">
            Add extra folders (e.g. one per household member) alongside the default library
          </p>
        </div>
        <div className="user-mgmt-actions">
          <Button variant="primary" onClick={() => setSheet({ kind: 'add' })}>
            Add library
          </Button>
          <Link className="user-mgmt-back-link" to="/settings">
            <ChevronRightIcon size={16} className="rotate-180" /> Back to settings
          </Link>
        </div>
      </div>

      {isLoading && <div className="user-mgmt-state">Loading libraries…</div>}
      {isError && (
        <div className="user-mgmt-state">
          Couldn't load libraries.
          <Button variant="primary" onClick={() => refetch()}>
            Try again
          </Button>
        </div>
      )}

      <div className="user-mgmt-list">
        {libraries.map((library) => (
          <Card key={library.id} className="user-mgmt-row">
            <div className="user-mgmt-row-info">
              <span className="user-mgmt-username">
                {library.name}
                {library.is_default && <Pill tone="accent">Default</Pill>}
              </span>
              <span className="user-mgmt-you">{library.path}</span>
            </div>
            <ActionMenu
              items={[
                {
                  label: scanningId === library.id ? 'Scanning…' : 'Scan now',
                  icon: <RefreshIcon size={16} className={scanningId === library.id ? 'spin' : undefined} />,
                  onClick: () => handleScan(library),
                  disabled: scanningId === library.id,
                },
                !library.is_default && {
                  label: 'Set as default',
                  onClick: () => handleSetDefault(library),
                  disabled: updateLibrary.isPending,
                },
                {
                  label: 'Delete library',
                  onClick: () => setSheet({ kind: 'delete', library }),
                  disabled: libraries.length <= 1,
                },
              ]}
            />
          </Card>
        ))}
      </div>

      <BottomSheet open={sheet?.kind === 'add'} onClose={closeAddSheet}>
        <h3 className="sheet-title">Add library</h3>
        <form onSubmit={handleCreate} className="user-mgmt-form">
          <label className="user-mgmt-field-label">
            Name
            <input value={name} onChange={(e) => setName(e.target.value)} required autoFocus />
          </label>
          <label className="user-mgmt-field-label">
            Folder path
            <input
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="/data/kids-books"
              required
            />
          </label>
          {formError && <p className="user-mgmt-field-error">{formError}</p>}
          <Button type="submit" disabled={createLibrary.isPending}>
            {createLibrary.isPending ? 'Creating…' : 'Create library'}
          </Button>
        </form>
      </BottomSheet>

      <BottomSheet open={sheet?.kind === 'delete'} onClose={() => setSheet(null)}>
        <h3 className="sheet-title">Delete library</h3>
        {sheet?.kind === 'delete' && (
          <>
            <p className="user-mgmt-confirm-text">
              Delete <strong>{sheet.library.name}</strong>? This removes its books from the catalog
              (files on disk are untouched) — this can't be undone.
            </p>
            <div className="user-mgmt-confirm-actions">
              <Button variant="ghost" onClick={() => setSheet(null)}>
                Cancel
              </Button>
              <Button variant="destructive" onClick={handleDelete} disabled={deleteLibrary.isPending}>
                {deleteLibrary.isPending ? 'Deleting…' : 'Delete'}
              </Button>
            </div>
          </>
        )}
      </BottomSheet>
    </div>
  );
}
