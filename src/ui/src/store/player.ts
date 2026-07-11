import { create } from 'zustand';
import type { AudioBookRow, FileMetadata } from '../types/book';

interface PlayerState {
  book: AudioBookRow | null;
  files: FileMetadata[];
  index: number;
  playing: boolean;
  currentTime: number;
  duration: number;
  rate: number;

  loadBook: (book: AudioBookRow, files: FileMetadata[]) => void;
  setIndex: (index: number) => void;
  reorderFiles: (files: FileMetadata[]) => void;
  setPlaying: (playing: boolean) => void;
  setTime: (currentTime: number, duration: number) => void;
  setRate: (rate: number) => void;
  clear: () => void;
}

export const usePlayerStore = create<PlayerState>((set, get) => ({
  book: null,
  files: [],
  index: 0,
  playing: false,
  currentTime: 0,
  duration: 0,
  rate: 1,

  loadBook: (book, files) => set({ book, files, currentTime: 0, duration: 0 }),
  setIndex: (index) => set({ index, currentTime: 0, duration: 0 }),
  // Preserves the current playback position — unlike setIndex, the file
  // playing is the same one, it may have just moved to a different slot.
  reorderFiles: (files) => {
    const currentId = get().files[get().index]?.id;
    const found = files.findIndex((f) => f.id === currentId);
    const newIndex = found === -1 ? Math.min(get().index, Math.max(files.length - 1, 0)) : found;
    set({ files, index: newIndex });
  },
  setPlaying: (playing) => set({ playing }),
  setTime: (currentTime, duration) => set({ currentTime, duration }),
  setRate: (rate) => set({ rate }),
  clear: () => set({ book: null, files: [], index: 0, playing: false, currentTime: 0, duration: 0 }),
}));
