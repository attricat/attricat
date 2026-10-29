import { create } from 'zustand';
import { maximumWatchedExtensionRuns } from './constants';

type RunWatchStore = {
  runIds: string[];
  /** Records whose host views reload when the run ends, by run ID. */
  records: Record<string, readonly string[]>;
  watch: (runId: string, recordIds?: readonly string[]) => void;
  unwatch: (runId: string) => void;
};

/**
 * Runs started in this tab. The host polls them until they finish and then
 * notifies the user, even after the starting extension frame has unmounted.
 */
export const useRunWatchStore = create<RunWatchStore>((set) => ({
  runIds: [],
  records: {},
  watch: (runId, recordIds = []) =>
    set((state) =>
      state.runIds.includes(runId)
        ? state
        : {
            runIds: [...state.runIds, runId].slice(
              -maximumWatchedExtensionRuns,
            ),
            records: { ...state.records, [runId]: recordIds },
          },
    ),
  unwatch: (runId) =>
    set((state) => ({
      runIds: state.runIds.filter((id) => id !== runId),
      records: Object.fromEntries(
        Object.entries(state.records).filter(([id]) => id !== runId),
      ),
    })),
}));
