import { useEffect, useState } from 'react';
import { BottomSheet } from '../ui/BottomSheet';
import { useSleepTimerStore } from '../../player/sleepTimer';
import { SleepIcon } from './icons';
import './player.css';

const PRESETS = [15, 30, 45, 60];

export function SleepTimerSheet() {
  const [open, setOpen] = useState(false);
  const endsAt = useSleepTimerStore((s) => s.endsAt);
  const endOfChapter = useSleepTimerStore((s) => s.endOfChapter);
  const start = useSleepTimerStore((s) => s.start);
  const startEndOfChapter = useSleepTimerStore((s) => s.startEndOfChapter);
  const cancel = useSleepTimerStore((s) => s.cancel);
  const [, forceTick] = useState(0);

  useEffect(() => {
    if (!endsAt) return;
    const id = setInterval(() => forceTick((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, [endsAt]);

  const remainingMin = endsAt ? Math.max(0, Math.ceil((endsAt - Date.now()) / 60_000)) : null;
  const active = endsAt !== null || endOfChapter;

  return (
    <>
      <button className="player-secondary-btn" onClick={() => setOpen(true)}>
        <SleepIcon />
        {active && <span className="player-secondary-badge" />}
      </button>
      <BottomSheet open={open} onClose={() => setOpen(false)}>
        <h3 className="sheet-title">Sleep timer</h3>
        {active ? (
          <>
            <p className="sleep-status">
              {endOfChapter ? 'Stopping at end of chapter' : `Stopping in ${remainingMin} min`}
            </p>
            <button
              className="btn btn-secondary"
              onClick={() => {
                cancel();
                setOpen(false);
              }}
            >
              Cancel
            </button>
          </>
        ) : (
          <div className="sleep-options">
            {PRESETS.map((m) => (
              <button
                key={m}
                className="sleep-option"
                onClick={() => {
                  start(m);
                  setOpen(false);
                }}
              >
                {m} min
              </button>
            ))}
            <button
              className="sleep-option"
              onClick={() => {
                startEndOfChapter();
                setOpen(false);
              }}
            >
              End of chapter
            </button>
          </div>
        )}
      </BottomSheet>
    </>
  );
}
