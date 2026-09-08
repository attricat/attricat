import { createContext } from 'react';
import type { ToastOptions } from './toast';

export type ToastContextValue = {
  dismiss: (id: number) => void;
  show: (options: ToastOptions) => void;
};

export const ToastContext = createContext<ToastContextValue | undefined>(
  undefined,
);
