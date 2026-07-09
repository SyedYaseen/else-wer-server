import { create } from 'zustand';
import { audio } from './engine';

interface SleepTimerState {
  endsAt: number | null;
  endOfChapter: boolean;
  start: (minutes: number) => void;
  startEndOfChapter: () => void;
  cancel: () => void;
}

let intervalId: number | null = null;

export const useSleepTimerStore = create<SleepTimerState>((set, get) => ({
  endsAt: null,
  endOfChapter: false,
  start: (minutes) => {
    if (intervalId) clearInterval(intervalId);
    set({ endsAt: Date.now() + minutes * 60_000, endOfChapter: false });
    intervalId = window.setInterval(() => {
      if (get().endsAt !== null && Date.now() >= get().endsAt!) {
        audio.pause();
        get().cancel();
      }
    }, 1000);
  },
  startEndOfChapter: () => {
    if (intervalId) {
      clearInterval(intervalId);
      intervalId = null;
    }
    set({ endsAt: null, endOfChapter: true });
  },
  cancel: () => {
    if (intervalId) {
      clearInterval(intervalId);
      intervalId = null;
    }
    set({ endsAt: null, endOfChapter: false });
  },
}));

// Called by the engine's `ended` handler — consumes the end-of-chapter flag so
// the current chapter's finish pauses playback instead of auto-advancing.
export function consumeEndOfChapter(): boolean {
  const { endOfChapter } = useSleepTimerStore.getState();
  if (endOfChapter) {
    useSleepTimerStore.getState().cancel();
    return true;
  }
  return false;
}
