import { useEffect, useState } from 'react';
import { BottomSheet } from '../ui/BottomSheet';
import { usePlayerStore } from '../../store/player';
import { switchToFile, seek } from '../../player/engine';
import { listBookmarks, createBookmark, deleteBookmark } from '../../api/bookmarks';
import type { Bookmark } from '../../types/book';
import { formatDuration } from '../../lib/format';
import { BookmarkIcon } from './icons';
import { TrashIcon } from '../ui/icons';
import './player.css';

export function BookmarksSheet() {
  const [open, setOpen] = useState(false);
  const [note, setNote] = useState('');
  const [adding, setAdding] = useState(false);
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([]);
  const book = usePlayerStore((s) => s.book);
  const files = usePlayerStore((s) => s.files);
  const index = usePlayerStore((s) => s.index);

  useEffect(() => {
    if (!open || !book) return;
    listBookmarks(book.id).then(setBookmarks).catch(() => {});
  }, [open, book]);

  async function handleAdd() {
    if (!book) return;
    const file = files[index];
    if (!file) return;
    setAdding(true);
    try {
      const bookmark = await createBookmark({
        book_id: book.id,
        file_id: file.id,
        timestamp_ms: Math.floor(usePlayerStore.getState().currentTime * 1000),
        note: note.trim() || undefined,
      });
      setBookmarks((prev) => [...prev, bookmark]);
      setNote('');
    } finally {
      setAdding(false);
    }
  }

  async function handleDelete(id: number) {
    await deleteBookmark(id);
    setBookmarks((prev) => prev.filter((b) => b.id !== id));
  }

  function handleJump(b: Bookmark) {
    const fileIndex = files.findIndex((f) => f.id === b.file_id);
    if (fileIndex === -1) return;
    if (fileIndex !== index) switchToFile(fileIndex);
    seek(b.timestamp_ms / 1000);
    setOpen(false);
  }

  return (
    <>
      <button
        className="player-secondary-btn"
        onClick={() => setOpen(true)}
        aria-label="Bookmarks"
        title="Bookmarks"
      >
        <BookmarkIcon />
      </button>
      <BottomSheet open={open} onClose={() => setOpen(false)}>
        <h3 className="sheet-title">Bookmarks</h3>
        <div className="bookmark-add-row">
          <input
            className="bookmark-note-input"
            type="text"
            placeholder="Add a note (optional)"
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
          <button className="bookmark-add-btn" onClick={handleAdd} disabled={adding}>
            Add bookmark
          </button>
        </div>
        <div className="bookmarks-list">
          {bookmarks.length === 0 && <p className="bookmarks-empty">No bookmarks yet.</p>}
          {bookmarks.map((b) => (
            <div key={b.id} className="bookmark-row">
              <button className="bookmark-row-main" onClick={() => handleJump(b)}>
                <span className="bookmark-timestamp">{formatDuration(b.timestamp_ms)}</span>
                {b.note && <span className="bookmark-note">{b.note}</span>}
              </button>
              <button
                className="icon-btn"
                aria-label="Delete bookmark"
                title="Delete bookmark"
                onClick={() => handleDelete(b.id)}
              >
                <TrashIcon />
              </button>
            </div>
          ))}
        </div>
      </BottomSheet>
    </>
  );
}
