import { useState } from 'react';
import { BottomSheet } from '../ui/BottomSheet';
import { ReorderChaptersSheet } from '../ui/ReorderChaptersSheet';
import { usePlayerStore } from '../../store/player';
import { switchToFile } from '../../player/engine';
import { formatDuration } from '../../lib/format';
import { ChaptersIcon, NowPlayingIcon } from './icons';
import { ReorderIcon } from '../ui/icons';
import './player.css';

export function ChaptersSheet() {
  const [open, setOpen] = useState(false);
  const [reordering, setReordering] = useState(false);
  const book = usePlayerStore((s) => s.book);
  const files = usePlayerStore((s) => s.files);
  const index = usePlayerStore((s) => s.index);

  return (
    <>
      <button
        className="player-secondary-btn"
        onClick={() => setOpen(true)}
        aria-label="Chapters"
        title="Chapters"
      >
        <ChaptersIcon />
      </button>
      <BottomSheet open={open} onClose={() => setOpen(false)}>
        <div className="chapters-sheet-header">
          <h3 className="sheet-title">Chapters</h3>
          <button
            className="icon-btn"
            aria-label="Reorder chapters"
            title="Reorder chapters"
            onClick={() => setReordering(true)}
          >
            <ReorderIcon />
          </button>
        </div>
        <div className="chapters-list">
          {files.map((f, i) => (
            <button
              key={f.id}
              className={`chapter-row ${i === index ? 'current' : ''} ${i < index ? 'played' : ''}`}
              onClick={() => {
                switchToFile(i);
                setOpen(false);
              }}
            >
              <span className="chapter-active-bar" />
              <span className="chapter-name">{f.file_name}</span>
              {f.duration ? <span className="chapter-duration">{formatDuration(f.duration)}</span> : null}
              {i === index && <NowPlayingIcon />}
            </button>
          ))}
        </div>
      </BottomSheet>
      {book && (
        <ReorderChaptersSheet
          open={reordering}
          onClose={() => setReordering(false)}
          bookId={book.id}
          onSaved={(newFiles) => usePlayerStore.getState().reorderFiles(newFiles)}
        />
      )}
    </>
  );
}
