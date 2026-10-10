import { create } from 'zustand';
import type { ActionDialogRequest } from './extensionBroker';

export type OpenActionDialog = ActionDialogRequest & {
  /** Distinguishes reopenings so the dialog frame always starts fresh. */
  key: number;
};

type ActionDialogStore = {
  dialog: OpenActionDialog | null;
  open: (request: ActionDialogRequest) => void;
  close: () => void;
};

let nextDialogKey = 0;

/**
 * The single host-managed extension action dialog. It lives outside the
 * source outlet, so clearing a selection or closing a menu never unmounts it.
 */
export const useActionDialogStore = create<ActionDialogStore>((set) => ({
  dialog: null,
  open: (request) => set({ dialog: { ...request, key: nextDialogKey++ } }),
  close: () => set({ dialog: null }),
}));
