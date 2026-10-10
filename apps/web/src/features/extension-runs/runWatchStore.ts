import { create } from 'zustand';
import { maximumWatchedExtensionRuns } from './constants';

type RunWatchStore = {
  runIds: string[];
  watch: (runId: string) => void;
  unwatch: (runId: string) => void;
};

/**
 * Runs started in this tab. The host polls them until they finish and then
 * notifies the user, even after the starting extension frame has unmounted.
 */
export const useRunWatchStore = create<RunWatchStore>((set) => ({
  runIds: [],
  watch: (runId) =>
    set((state) =>
      state.runIds.includes(runId)
        ? state
        : {
            runIds: [...state.runIds, runId].slice(
              -maximumWatchedExtensionRuns,
            ),
          },
    ),
  unwatch: (runId) =>
    set((state) => ({ runIds: state.runIds.filter((id) => id !== runId) })),
}));
