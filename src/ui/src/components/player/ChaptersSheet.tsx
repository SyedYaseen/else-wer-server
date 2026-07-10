import { useState } from 'react';
import { BottomSheet } from '../ui/BottomSheet';
import { usePlayerStore } from '../../store/player';
import { switchToFile } from '../../player/engine';
import { formatDuration } from '../../lib/format';
import { ChaptersIcon, NowPlayingIcon } from './icons';
import './player.css';

export function ChaptersSheet() {
  const [open, setOpen] = useState(false);
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
        <h3 className="sheet-title">Chapters</h3>
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
    </>
  );
}
