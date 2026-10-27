import { queryOptions } from '@tanstack/react-query';
import {
  listConnections,
  recordExportStatus,
  workspaceExportStatus,
} from './api';
import { extensionExportQueryKeys } from './queryKeys';

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
  });

/** The connections an extension exports through. */
export const extensionConnectionsOptions = (extensionId: string) =>
  queryOptions({
    queryKey: extensionExportQueryKeys.connections(extensionId),
    queryFn: () => listConnections(extensionId),
  });
