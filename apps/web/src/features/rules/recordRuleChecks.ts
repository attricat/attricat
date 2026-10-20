import { create } from 'zustand';
import { RECORD_RULE_CHECK_WATCH_MS } from './constants';

type RecordRuleChecksStore = {
  /** When each edited record stops waiting for its rule runs, in epoch ms. */
  watchUntil: Readonly<Record<string, number>>;
  watch: (recordId: string) => void;
};

/**
 * Records whose edit has just been saved. Rule runs start from the edit's
 * events, after the save returns, so the record page keeps checking for them
 * for a short while.
 */
export const useRecordRuleChecks = create<RecordRuleChecksStore>((set) => ({
  watchUntil: {},
  watch: (recordId) =>
    set((state) => ({
      watchUntil: {
        ...state.watchUntil,
        [recordId]: Date.now() + RECORD_RULE_CHECK_WATCH_MS,
      },
    })),
}));

export const watchRecordRuleChecks = (recordId: string) =>
  useRecordRuleChecks.getState().watch(recordId);

export const isWatchingRecordRuleChecks = (recordId: string) =>
  (useRecordRuleChecks.getState().watchUntil[recordId] ?? 0) > Date.now();
