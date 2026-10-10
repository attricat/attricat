import { create } from 'zustand';
import type { DesignMode } from './theme';

const storageKey = 'attricat.color-mode';

const savedMode = (): DesignMode | null => {
  try {
    const value = localStorage.getItem(storageKey);
    return value === 'light' || value === 'dark' ? value : null;
  } catch {
    return null;
  }
};

type ColorModeStore = {
  preference: DesignMode | null;
  setPreference: (mode: DesignMode) => void;
};

export const useColorMode = create<ColorModeStore>((set) => ({
  preference: savedMode(),
  setPreference: (mode) => {
    try {
      localStorage.setItem(storageKey, mode);
    } catch {
      // The mode switch still works when storage is unavailable.
    }
    set({ preference: mode });
  },
}));
