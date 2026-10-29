import { create } from 'zustand';
import { reduceToasts, type Toast } from './toastQueue';
import type { ToastOptions } from './toast';

type ToastStore = {
  toasts: Toast[];
  /** Shows a toast and returns its ID, for {@link ToastStore.dismiss}. */
  show: (options: ToastOptions) => number;
  dismiss: (id: number) => void;
};

let nextToastId = 0;

export const useToastStore = create<ToastStore>((set) => ({
  toasts: [],
  show: (options) => {
    const id = nextToastId++;
    set((state) => ({
      toasts: reduceToasts(state.toasts, {
        toast: { ...options, id, severity: options.severity ?? 'info' },
        type: 'show',
      }),
    }));
    return id;
  },
  dismiss: (id) =>
    set((state) => ({
      toasts: reduceToasts(state.toasts, { id, type: 'dismiss' }),
    })),
}));
