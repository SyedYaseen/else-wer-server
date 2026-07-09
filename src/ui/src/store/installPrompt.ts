import { create } from 'zustand';

export interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>;
}

interface InstallPromptState {
  deferredEvent: BeforeInstallPromptEvent | null;
  installed: boolean;
  setDeferredEvent: (e: BeforeInstallPromptEvent | null) => void;
  setInstalled: (v: boolean) => void;
}

export const useInstallPromptStore = create<InstallPromptState>((set) => ({
  deferredEvent: null,
  installed: false,
  setDeferredEvent: (deferredEvent) => set({ deferredEvent }),
  setInstalled: (installed) => set({ installed, deferredEvent: null }),
}));
