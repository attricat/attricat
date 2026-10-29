import { useEffect } from 'react';
import { create } from 'zustand';
import type { ActionsProps } from './useOfferedActions';

/** An action surface the current page shows, offered in the palette too. */
export type PaletteSurface = Omit<ActionsProps, 'states'>;

type ActionPaletteStore = {
  surfaces: Record<string, PaletteSurface>;
  open: boolean;
  register: (id: string, surface: PaletteSurface) => void;
  unregister: (id: string) => void;
  setOpen: (open: boolean) => void;
};

export const useActionPaletteStore = create<ActionPaletteStore>((set) => ({
  surfaces: {},
  open: false,
  register: (id, surface) =>
    set((state) => ({ surfaces: { ...state.surfaces, [id]: surface } })),
  unregister: (id) =>
    set((state) => ({
      surfaces: Object.fromEntries(
        Object.entries(state.surfaces).filter(([key]) => key !== id),
      ),
    })),
  setOpen: (open) => set({ open }),
}));

/**
 * Offers a surface's actions in the command palette while the calling
 * component is mounted; `surface` undefined offers nothing.
 */
export const usePaletteSurface = (
  id: string,
  surface: PaletteSurface | undefined,
) => {
  const register = useActionPaletteStore((state) => state.register);
  const unregister = useActionPaletteStore((state) => state.unregister);
  // Surfaces are rebuilt each render; their content decides re-registering.
  const key = surface ? JSON.stringify(surface) : '';
  useEffect(() => {
    if (!key) return;
    register(id, JSON.parse(key) as PaletteSurface);
    return () => unregister(id);
  }, [id, key, register, unregister]);
};
