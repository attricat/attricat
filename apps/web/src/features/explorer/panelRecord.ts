import { createContext, use, useSyncExternalStore } from 'react';

/**
 * The record open in the Explorer's panel. It lives in the URL, but opening
 * or closing the panel must not re-render the results: only the panel and the
 * rows whose highlight changes subscribe to it.
 */
type PanelRecordStore = {
  get: () => string | undefined;
  subscribe: (listener: () => void) => () => void;
};

export const createPanelRecordStore = (initial: string | undefined) => {
  let recordId = initial;
  const listeners = new Set<() => void>();
  return {
    get: () => recordId,
    set: (next: string | undefined) => {
      if (next === recordId) return;
      recordId = next;
      listeners.forEach((listener) => listener());
    },
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
};

export const PanelRecordContext = createContext<PanelRecordStore>({
  get: () => undefined,
  subscribe: () => () => {},
});

/** The record in the panel, re-rendering whenever it changes. */
export const usePanelRecordId = () => {
  const store = use(PanelRecordContext);
  return useSyncExternalStore(store.subscribe, store.get);
};

/** Whether `recordId` is in the panel, re-rendering only when that changes. */
export const useIsPanelRecord = (recordId: string) => {
  const store = use(PanelRecordContext);
  return useSyncExternalStore(store.subscribe, () => store.get() === recordId);
};

/** Reads the panel record when an event needs it, without subscribing. */
export const usePanelRecordReader = () => use(PanelRecordContext).get;
