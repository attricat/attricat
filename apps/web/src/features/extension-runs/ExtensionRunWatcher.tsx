import { useQueries } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useEffect } from 'react';
import { extensionRunDetailOptions } from './queryOptions';
import { isActiveExtensionRun } from './runPolling';
import { notifyRunFinished } from './runNotifications';
import { useRunWatchStore } from './runWatchStore';

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
    });
  }, [navigate, runIds, runs, unwatch]);
  return null;
};
