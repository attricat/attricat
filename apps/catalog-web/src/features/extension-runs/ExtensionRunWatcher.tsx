import { useQueries } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useEffect } from 'react';
import { getExtensionRun } from './api';
import {
  activeExtensionRunStatuses,
  extensionRunPollMilliseconds,
} from './constants';
import { extensionRunQueryKeys } from './queryKeys';
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
    queries: runIds.map((runId) => ({
      queryKey: extensionRunQueryKeys.detail(runId),
      queryFn: () => getExtensionRun(runId),
      refetchInterval: (query: { state: { data?: { status: string } } }) =>
        query.state.data &&
        !activeExtensionRunStatuses.some(
          (status) => status === query.state.data?.status,
        )
          ? false
          : extensionRunPollMilliseconds,
    })),
  });
  useEffect(() => {
    runs.forEach((result, index) => {
      const runId = runIds[index];
      if (result.isError) {
        unwatch(runId);
        return;
      }
      const run = result.data;
      if (!run || activeExtensionRunStatuses.includes(run.status)) return;
      unwatch(runId);
      if (announcedRuns.has(runId)) return;
      announcedRuns.add(runId);
      notifyRunFinished(run, navigate);
    });
  }, [navigate, runIds, runs, unwatch]);
  return null;
};
