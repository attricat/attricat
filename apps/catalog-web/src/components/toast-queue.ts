import type { AlertColor } from '@mui/material';
import type { ToastOptions } from './toast';

export type Toast = ToastOptions & {
  count: number;
  id: number;
  severity: AlertColor;
};

export type ToastAction =
  | { toast: Omit<Toast, 'count'>; type: 'show' }
  | { id: number; type: 'dismiss' };

const defaultDedupeKey = ({ message, severity = 'info' }: ToastOptions) =>
  `${severity}:${message}`;

export const reduceToasts = (state: Toast[], action: ToastAction): Toast[] => {
  if (action.type === 'dismiss')
    return state.filter((toast) => toast.id !== action.id);

  const dedupeKey = action.toast.dedupeKey ?? defaultDedupeKey(action.toast);
  const existing = state.find(
    (toast) => (toast.dedupeKey ?? defaultDedupeKey(toast)) === dedupeKey,
  );
  if (!existing) return [...state, { ...action.toast, count: 1, dedupeKey }];

  return state.map((toast) =>
    toast.id === existing.id
      ? {
          ...toast,
          ...action.toast,
          count: toast.count + 1,
          dedupeKey,
          id: toast.id,
        }
      : toast,
  );
};
