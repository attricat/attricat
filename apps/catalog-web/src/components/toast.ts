import type { AlertColor } from '@mui/material';
import type { ReactNode } from 'react';

export type ToastOptions = {
  action?: ReactNode;
  autoHideDuration?: number | null;
  /** Repeated notifications with this key are grouped and counted. */
  dedupeKey?: string;
  message: string;
  severity?: AlertColor;
};

const subscribers = new Set<(options: ToastOptions) => void>();
const pendingToasts: ToastOptions[] = [];

export const subscribeToToasts = (
  subscriber: (options: ToastOptions) => void,
) => {
  subscribers.add(subscriber);
  pendingToasts.splice(0).forEach(subscriber);
  return () => subscribers.delete(subscriber);
};

/**
 * Imperative notification API for code that cannot use React hooks, including
 * extension bridges. React components should prefer useToast().
 */
export const toast = {
  error: (
    message: string,
    options: Omit<ToastOptions, 'message' | 'severity'> = {},
  ) => toast.show({ ...options, message, severity: 'error' }),
  info: (
    message: string,
    options: Omit<ToastOptions, 'message' | 'severity'> = {},
  ) => toast.show({ ...options, message, severity: 'info' }),
  show: (options: ToastOptions) => {
    if (!subscribers.size) {
      pendingToasts.push(options);
      return;
    }
    subscribers.forEach((subscriber) => subscriber(options));
  },
  success: (
    message: string,
    options: Omit<ToastOptions, 'message' | 'severity'> = {},
  ) => toast.show({ ...options, message, severity: 'success' }),
  warning: (
    message: string,
    options: Omit<ToastOptions, 'message' | 'severity'> = {},
  ) => toast.show({ ...options, message, severity: 'warning' }),
};
