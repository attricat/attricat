import { useQueries, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useEffect } from 'react';
import { extensionRunDetailOptions } from './queryOptions';
import { isActiveExtensionRun } from './runPolling';
import { notifyRunFinished } from './runNotifications';
import { useRunWatchStore } from './runWatchStore';
import { refreshRecord } from '../extensions/refreshRecord';
import { recordQueryKeys } from '../records/queryKeys';

// Effects can replay (for example in development Strict Mode); announce each
// terminal run once per tab.
const announcedRuns = new Set<string>();

/**
 * Polls runs started in this tab only while they are active, then announces
 * the outcome. Mounted once in the application shell so notifications do not
 * depend on the extension frame that started the run.
 */
export const ExtensionRunWatcher = () => {
  const runIds = useRunWatchStore((state) => state.runIds);
  const records = useRunWatchStore((state) => state.records);
  const queryClient = useQueryClient();
  const unwatch = useRunWatchStore((state) => state.unwatch);
  const navigate = useNavigate();
  const runs = useQueries({
    queries: runIds.map(extensionRunDetailOptions),
  });
  useEffect(() => {
    runs.forEach((result, index) => {
      const runId = runIds[index];
      if (result.isError) {
        unwatch(runId);
        return;
      }
      const run = result.data;
      if (!run || isActiveExtensionRun(run)) return;
      unwatch(runId);
      if (announcedRuns.has(runId)) return;
      announcedRuns.add(runId);
      notifyRunFinished(run, navigate);
      // A finished run may have changed its records; show their new state.
      const changed = records[runId] ?? [];
      if (changed.length)
        void Promise.all([
          ...changed.map((recordId) => refreshRecord(queryClient, recordId)),
          queryClient.invalidateQueries({
            queryKey: recordQueryKeys.searches(),
          }),
        ]);
    });
  }, [navigate, queryClient, records, runIds, runs, unwatch]);
  return null;
};
