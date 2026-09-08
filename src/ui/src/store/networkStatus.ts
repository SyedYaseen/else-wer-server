import { create } from 'zustand';

interface NetworkStatusState {
  reachable: boolean;
  setReachable: (reachable: boolean) => void;
  // Whether every library's root directory is reachable on the server, per the
  // /api/health payload. null = not yet known, or the server couldn't determine it.
  // Deliberately separate from `reachable` rather than folded into one enum:
  // api/client.ts writes `reachable` on any fetch throw and knows nothing about
  // media state, so the two must move independently.
  mediaOk: boolean | null;
  setMediaOk: (mediaOk: boolean | null) => void;
}

// Optimistic initial state: never flash "offline" before the first probe resolves.
export const useNetworkStatusStore = create<NetworkStatusState>((set) => ({
  reachable: true,
  setReachable: (reachable) => set({ reachable }),
  mediaOk: null,
  setMediaOk: (mediaOk) => set({ mediaOk }),
}));
