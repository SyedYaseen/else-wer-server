import { create } from 'zustand';

interface NetworkStatusState {
  reachable: boolean;
  setReachable: (reachable: boolean) => void;
}

// Optimistic initial state: never flash "offline" before the first probe resolves.
export const useNetworkStatusStore = create<NetworkStatusState>((set) => ({
  reachable: true,
  setReachable: (reachable) => set({ reachable }),
}));
