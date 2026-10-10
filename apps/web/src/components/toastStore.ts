import { create } from 'zustand';
import { reduceToasts, type Toast } from './toastQueue';
import type { ToastOptions } from './toast';

type ToastStore = {
  toasts: Toast[];
  show: (options: ToastOptions) => void;
  dismiss: (id: number) => void;
};

let nextToastId = 0;

export const useToastStore = create<ToastStore>((set) => ({
  toasts: [],
  show: (options) =>
    set((state) => ({
      toasts: reduceToasts(state.toasts, {
        toast: {
          ...options,
          id: nextToastId++,
          severity: options.severity ?? 'info',
        },
        type: 'show',
      }),
    })),
  dismiss: (id) =>
    set((state) => ({
      toasts: reduceToasts(state.toasts, { id, type: 'dismiss' }),
    })),
}));
