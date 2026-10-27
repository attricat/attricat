import { create } from 'zustand';
import type { DesignMode } from './theme';
import {
  readStored,
  writeStored,
  storageAreas,
} from '../components/browserStorage';

const storageKey = 'attricat.color-mode';

const savedMode = (): DesignMode | null => {
  const value = readStored(storageAreas.local, storageKey);
  return value === 'light' || value === 'dark' ? value : null;
};

type ColorModeStore = {
  preference: DesignMode | null;
  setPreference: (mode: DesignMode) => void;
};

export const useColorMode = create<ColorModeStore>((set) => ({
  preference: savedMode(),
  setPreference: (mode) => {
    writeStored(storageAreas.local, storageKey, mode);
    set({ preference: mode });
  },
}));
