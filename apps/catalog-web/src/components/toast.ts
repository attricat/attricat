import type { AlertColor } from '@mui/material';
import type { ReactNode } from 'react';
import { useToastStore } from './toast-store';

export type ToastOptions = {
  action?: ReactNode;
  autoHideDuration?: number | null;
  /** Repeated notifications with this key are grouped and counted. */
  dedupeKey?: string;
  message: string;
  severity?: AlertColor;
};

/**
 * Imperative notification API for code that cannot use React hooks, including
 * extension bridges. React components should prefer useToast(). Notifications
 * emitted before the UI mounts remain queued.
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
  show: (options: ToastOptions) => useToastStore.getState().show(options),
  success: (
    message: string,
    options: Omit<ToastOptions, 'message' | 'severity'> = {},
  ) => toast.show({ ...options, message, severity: 'success' }),
  warning: (
    message: string,
    options: Omit<ToastOptions, 'message' | 'severity'> = {},
  ) => toast.show({ ...options, message, severity: 'warning' }),
};
