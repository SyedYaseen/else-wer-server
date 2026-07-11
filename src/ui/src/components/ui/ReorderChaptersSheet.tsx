import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { BottomSheet } from './BottomSheet';
import { Button } from './Button';
import { ArrowUpIcon, ArrowDownIcon } from './icons';
import { formatDuration } from '../../lib/format';
import { fileMetadata, reorderFiles } from '../../api/books';
import type { FileMetadata } from '../../types/book';

interface ReorderChaptersSheetProps {
  open: boolean;
  onClose: () => void;
  bookId: number;
  // Player passes this to sync usePlayerStore immediately instead of
  // reloading; Organize has nothing else that depends on chapter order so it
  // can omit it.
  onSaved?: (files: FileMetadata[]) => void;
}

// Always fetches its own file list by bookId (same endpoint used to order the
// player's chapter list — disc_number/track_number/file_name) rather than
// trusting a caller-supplied list, since the organize tree's file order is
// scan/insertion order, not playback order.
export function ReorderChaptersSheet({ open, onClose, bookId, onSaved }: ReorderChaptersSheetProps) {
  const queryClient = useQueryClient();
  const { data, isLoading, isError } = useQuery({
    queryKey: ['fileMetadata', bookId],
    queryFn: () => fileMetadata(bookId),
    enabled: open,
  });
  const [order, setOrder] = useState<FileMetadata[]>([]);

  useEffect(() => {
    if (data) setOrder(data);
  }, [data]);

  const save = useMutation({
    mutationFn: () => reorderFiles(bookId, order.map((f) => f.id)),
    onSuccess: (newFiles) => {
      queryClient.setQueryData(['fileMetadata', bookId], newFiles);
      onSaved?.(newFiles);
      onClose();
    },
  });

  function move(index: number, delta: number) {
    setOrder((prev) => {
      const target = index + delta;
      if (target < 0 || target >= prev.length) return prev;
      const next = [...prev];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  }

  if (!open) return null;

  return (
    <BottomSheet open={open} onClose={onClose}>
      <h3 className="sheet-title">Reorder chapters</h3>
      {isLoading && <div className="reorder-state">Loading…</div>}
      {isError && <div className="reorder-state">Couldn't load chapters.</div>}
      {!isLoading && !isError && (
        <>
          <div className="reorder-list">
            {order.map((f, i) => (
              <div key={f.id} className="reorder-row">
                <span className="reorder-index">{i + 1}</span>
                <span className="reorder-name">{f.file_name}</span>
                {f.duration ? <span className="reorder-duration">{formatDuration(f.duration)}</span> : null}
                <div className="reorder-controls">
                  <button
                    className="icon-btn"
                    aria-label="Move up"
                    title="Move up"
                    disabled={i === 0}
                    onClick={() => move(i, -1)}
                  >
                    <ArrowUpIcon />
                  </button>
                  <button
                    className="icon-btn"
                    aria-label="Move down"
                    title="Move down"
                    disabled={i === order.length - 1}
                    onClick={() => move(i, 1)}
                  >
                    <ArrowDownIcon />
                  </button>
                </div>
              </div>
            ))}
          </div>
          {save.isError && <div className="reorder-error">Couldn't save the new order. Try again.</div>}
          <Button onClick={() => save.mutate()} disabled={save.isPending}>
            {save.isPending ? 'Saving…' : 'Save order'}
          </Button>
        </>
      )}
    </BottomSheet>
  );
}
