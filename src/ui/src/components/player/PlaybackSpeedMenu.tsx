import { useState } from 'react';
import { BottomSheet } from '../ui/BottomSheet';
import { usePlayerStore } from '../../store/player';
import { setRate } from '../../player/engine';
import { SpeedIcon } from './icons';
import './player.css';

const SPEEDS = [...new Set([0.25, 0.75, ...Array.from({ length: 28 }, (_, i) => (i + 3) / 10)])].sort(
  (a, b) => a - b,
);

export function PlaybackSpeedMenu() {
  const [open, setOpen] = useState(false);
  const rate = usePlayerStore((s) => s.rate);
  const label = `Playback speed, ${rate}x`;

  return (
    <>
      <button
        className="player-secondary-btn"
        onClick={() => setOpen(true)}
        aria-label={label}
        title={label}
      >
        <SpeedIcon size={18} />
        <span className="player-speed-label">{rate}x</span>
      </button>
      <BottomSheet open={open} onClose={() => setOpen(false)}>
        <h3 className="sheet-title">Playback speed</h3>
        <div className="speed-options">
          {SPEEDS.map((s) => (
            <button
              key={s}
              className={`speed-option ${s === rate ? 'active' : ''}`}
              onClick={() => {
                setRate(s);
                setOpen(false);
              }}
            >
              {s}x
            </button>
          ))}
        </div>
      </BottomSheet>
    </>
  );
}
