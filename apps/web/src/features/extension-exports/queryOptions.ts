import { queryOptions } from '@tanstack/react-query';
import {
  listConnections,
  recordExportStatus,
  workspaceExportStatus,
} from './api';
import {
  activeExportStatuses,
  exportStatusPollMilliseconds,
} from './constants';
import type { ExportStatus } from './constants';
import { extensionExportQueryKeys } from './queryKeys';

/**
 * Pushes run in the background and nothing announces their result, so a view
 * refreshes while it shows one still running. A failed request stops it.
 */
export const exportStatusRefetchInterval = (
  statuses: readonly ExportStatus[],
  error: unknown,
) =>
  !error && statuses.some((status) => activeExportStatuses.includes(status))
    ? exportStatusPollMilliseconds
    : false;

/** How the workspace's records are exported, across every export. */
export const workspaceExportStatusOptions = () =>
  queryOptions({
    queryKey: extensionExportQueryKeys.workspace(),
    queryFn: workspaceExportStatus,
  });

/** Where one record is exported and how each export went. */
export const recordExportStatusOptions = (recordId: string) =>
  queryOptions({
    queryKey: extensionExportQueryKeys.record(recordId),
    queryFn: () => recordExportStatus(recordId),
    refetchInterval: ({ state: { data, error } }) =>
      exportStatusRefetchInterval(
        data?.statuses.map((mapping) => mapping.status) ?? [],
        error,
      ),
  });

/** The connections an extension exports through. */
export const extensionConnectionsOptions = (extensionId: string) =>
  queryOptions({
    queryKey: extensionExportQueryKeys.connections(extensionId),
    queryFn: () => listConnections(extensionId),
  });
