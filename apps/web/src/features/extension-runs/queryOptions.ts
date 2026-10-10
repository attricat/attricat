import { queryOptions } from '@tanstack/react-query';
import { getExtensionRun, listExtensionRuns } from './api';
import { extensionRunQueryKeys } from './queryKeys';
import {
  extensionRunRefetchInterval,
  isActiveExtensionRun,
} from './runPolling';

/** One run, polled until it finishes. */
export const extensionRunDetailOptions = (runId: string) =>
  queryOptions({
    queryKey: extensionRunQueryKeys.detail(runId),
    queryFn: () => getExtensionRun(runId),
    refetchInterval: ({ state: { data, error } }) =>
      extensionRunRefetchInterval(!data || isActiveExtensionRun(data), error),
  });

/** The signed-in user's recent runs, polled while any of them is active. */
export const extensionRunListOptions = () =>
  queryOptions({
    queryKey: extensionRunQueryKeys.list(),
    queryFn: () => listExtensionRuns(),
    refetchInterval: ({ state: { data, error } }) =>
      extensionRunRefetchInterval(
        Boolean(data?.some(isActiveExtensionRun)),
        error,
      ),
  });
