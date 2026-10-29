import { create } from 'zustand';
import type { ActionContext } from './actions';

/** One of an extension's dialogs, opened by an action with its context. */
export type ActionDialogRequest = {
  extensionId: string;
  dialogId: string;
  context: ActionContext;
};

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
