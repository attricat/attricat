import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
import {
  exportStatusPath,
  exportStatusPageSize,
  extensionPath,
  recordExportStatusPath,
  type ConnectionGrantType,
  type ExportStatus,
} from './constants';
import {
  extensionConnectionSchema,
  extensionExportStatusSchema,
  hostPermissionBudgetSchema,
  previewResponseSchema,
  recordExportStatusSchema,
  retryResponseSchema,
  workspaceExportStatusSchema,
} from './schemas';

const json = (value: unknown, method = 'POST'): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(value),
});

export const recordExportStatus = (recordId: string) =>
  request(recordExportStatusPath(recordId), recordExportStatusSchema);

export const workspaceExportStatus = () =>
  request(exportStatusPath, workspaceExportStatusSchema);

export const extensionExportStatus = (
  extensionId: string,
  filter: {
    connection: string | null;
    status: ExportStatus | null;
    after?: string | null;
  },
) => {
  const params = new URLSearchParams({ limit: String(exportStatusPageSize) });
  if (filter.connection) params.set('connection', filter.connection);
  if (filter.status) params.set('status', filter.status);
  if (filter.after) params.set('after', filter.after);
  return request(
    `${extensionPath(extensionId)}/export-status?${params}`,
    extensionExportStatusSchema,
  );
};

export const retryExports = (
  extensionId: string,
  selection: {
    connection: string | null;
    recordIds?: string[];
    status?: ExportStatus;
  },
) =>
  request(
    `${extensionPath(extensionId)}/export-status/retry`,
    retryResponseSchema,
    // Retried records are also resynced at once by the export jobs of
    // their connection, so a retry pushes even without a retry handler.
    json({
      connection: selection.connection,
      run: true,
      ...(selection.recordIds ? { record_ids: selection.recordIds } : {}),
      ...(selection.status ? { status: selection.status } : {}),
    }),
  );

export const previewExport = (
  extensionId: string,
  exportId: string,
  recordId: string,
  connection: string | null,
) =>
  request(
    `${extensionPath(extensionId)}/exports/${encodeURIComponent(exportId)}/preview`,
    previewResponseSchema,
    json({ record_id: recordId, connection }),
  );

export const listConnections = (extensionId: string) =>
  request(
    `${extensionPath(extensionId)}/connections`,
    z.array(extensionConnectionSchema),
  );

export type NewConnection = {
  connection: string;
  kind: string;
  display_name: string;
  grant_type: ConnectionGrantType;
  token_url: string;
  device_authorization_url?: string;
  client_id: string;
  scope?: string;
  parameters?: Record<string, unknown>;
  client_secret?: string;
  refresh_token?: string;
};

export const createConnection = (extensionId: string, input: NewConnection) =>
  request(
    `${extensionPath(extensionId)}/connections`,
    extensionConnectionSchema,
    json(input),
  );

const connectionPath = (extensionId: string, connection: string) =>
  `${extensionPath(extensionId)}/connections/${encodeURIComponent(connection)}`;

export const updateConnection = (
  extensionId: string,
  connection: string,
  patch: {
    display_name?: string;
    client_secret?: string;
    refresh_token?: string;
  },
) =>
  request(
    connectionPath(extensionId, connection),
    extensionConnectionSchema,
    json(patch, 'PATCH'),
  );

export const deleteConnection = (extensionId: string, connection: string) =>
  requestNoContent(connectionPath(extensionId, connection), {
    method: 'DELETE',
  });

export const connectionAction = (
  extensionId: string,
  connection: string,
  action: 'device-authorization' | 'device-authorization/poll' | 'test',
) =>
  request(
    `${connectionPath(extensionId, connection)}/${action}`,
    extensionConnectionSchema,
    { method: 'POST' },
  );

export const exchangeAuthorizationCode = (
  extensionId: string,
  connection: string,
  input: { code: string; redirect_uri: string },
) =>
  request(
    `${connectionPath(extensionId, connection)}/authorization-code`,
    extensionConnectionSchema,
    json(input),
  );

export const listBudgets = (extensionId: string) =>
  request(
    `${extensionPath(extensionId)}/host-permission-budgets`,
    z.array(hostPermissionBudgetSchema),
  );

const budgetPath = (extensionId: string, hostPermissionId: string) =>
  `${extensionPath(extensionId)}/host-permission-budgets/${encodeURIComponent(hostPermissionId)}`;

export const setBudget = (
  extensionId: string,
  hostPermissionId: string,
  budget: { requests: number; per_seconds: number },
) =>
  requestNoContent(
    budgetPath(extensionId, hostPermissionId),
    json(budget, 'PUT'),
  );

export const clearBudget = (extensionId: string, hostPermissionId: string) =>
  requestNoContent(budgetPath(extensionId, hostPermissionId), {
    method: 'DELETE',
  });
