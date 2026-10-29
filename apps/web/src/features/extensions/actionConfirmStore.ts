import { create } from 'zustand';

type PendingConfirmation = {
  title: string;
  message: string;
  destructive: boolean;
  resolve: (confirmed: boolean) => void;
};

type ActionConfirmStore = {
  pending: PendingConfirmation | null;
  /** Asks the user before an action runs; resolves to whether they agreed. */
  ask: (request: Omit<PendingConfirmation, 'resolve'>) => Promise<boolean>;
  answer: (confirmed: boolean) => void;
};

/**
 * The single host-owned confirmation shown before an extension action runs.
 * A second request answers the first as declined.
 */
export const useActionConfirmStore = create<ActionConfirmStore>((set, get) => ({
  pending: null,
  ask: (request) =>
    new Promise<boolean>((resolve) => {
      get().pending?.resolve(false);
      set({ pending: { ...request, resolve } });
    }),
  answer: (confirmed) => {
    get().pending?.resolve(confirmed);
    set({ pending: null });
  },
}));
