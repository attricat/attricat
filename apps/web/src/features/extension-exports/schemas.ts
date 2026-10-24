import { z } from 'zod';
import { connectionGrantTypes, exportStatuses } from './constants';

export const exportMappingSchema = z.object({
  extension_id: z.string(),
  connection: z.string().nullable(),
  record_id: z.uuid(),
  external_id: z.string().nullable(),
  external_ids: z.record(z.string(), z.unknown()),
  content_hash: z.string().nullable(),
  record_revision: z.number().int(),
  status: z.enum(exportStatuses),
  error: z
    .looseObject({
      code: z.string().optional(),
      message: z.string().optional(),
    })
    .nullable(),
  attempts: z.number().int(),
  retry_requested_at: z.string().nullable(),
  synced_at: z.string().nullable(),
  updated_at: z.string(),
});
export type ExportMapping = z.infer<typeof exportMappingSchema>;

export const exportDestinationSchema = z.object({
  extension_id: z.string(),
  extension_name: z.string(),
  export_id: z.string(),
  title: z.string(),
  connection_kind: z.string().nullable(),
  preview: z.boolean(),
  resync_operation: z.string().nullable(),
  reconcile_operation: z.string().nullable(),
});
export type ExportDestination = z.infer<typeof exportDestinationSchema>;

export const connectionLabelSchema = z.object({
  extension_id: z.string(),
  connection: z.string(),
  kind: z.string(),
  display_name: z.string(),
});
export type ConnectionLabel = z.infer<typeof connectionLabelSchema>;

export const recordExportStatusSchema = z.object({
  statuses: z.array(exportMappingSchema),
  destinations: z.array(exportDestinationSchema),
  connections: z.array(connectionLabelSchema).default([]),
});
export type RecordExportStatus = z.infer<typeof recordExportStatusSchema>;

export const exportStatusCountSchema = z.object({
  extension_id: z.string(),
  connection: z.string().nullable(),
  status: z.enum(exportStatuses),
  count: z.number().int(),
});
export type ExportStatusCount = z.infer<typeof exportStatusCountSchema>;

export const workspaceExportStatusSchema = z.object({
  counts: z.array(exportStatusCountSchema),
  destinations: z.array(exportDestinationSchema),
});

export const handlerFailureSchema = z.object({
  event_id: z.uuid(),
  event_type: z.string(),
  handler_id: z.string(),
  status: z.string(),
  attempts: z.number().int(),
  error_code: z.string().nullable(),
  error_message: z.string().nullable(),
  record_id: z.uuid().nullable(),
  updated_at: z.string(),
});
export type HandlerFailure = z.infer<typeof handlerFailureSchema>;

export const extensionExportStatusSchema = z.object({
  counts: z.array(exportStatusCountSchema),
  mappings: z.array(exportMappingSchema),
  next: z.uuid().nullable(),
  handler_failures: z.array(handlerFailureSchema),
});
export type ExtensionExportStatus = z.infer<typeof extensionExportStatusSchema>;

export const extensionConnectionSchema = z.object({
  id: z.uuid(),
  extension_id: z.string(),
  connection: z.string(),
  kind: z.string(),
  display_name: z.string(),
  grant_type: z.enum(connectionGrantTypes),
  host_permission_id: z.string(),
  token_url: z.string(),
  device_authorization_url: z.string().nullable(),
  client_id: z.string(),
  scope: z.string().nullable(),
  parameters: z.record(z.string(), z.unknown()),
  status: z.enum(['pending_authorization', 'active', 'failed', 'revoked']),
  last_error: z.string().nullable(),
  last_refreshed_at: z.string().nullable(),
  access_token_expires_at: z.string().nullable(),
  device_user_code: z.string().nullable(),
  device_verification_uri: z.string().nullable(),
  device_expires_at: z.string().nullable(),
  authorization_url: z.string().nullable(),
  authorization_expires_at: z.string().nullable(),
  /** Set for authorization-code connections when browser authorization is configured. */
  redirect_uri: z.string().optional(),
  has_client_secret: z.boolean(),
  has_refresh_token: z.boolean(),
  has_access_token: z.boolean(),
  created_at: z.string(),
  updated_at: z.string(),
});
export type ExtensionConnection = z.infer<typeof extensionConnectionSchema>;

const rateLimitSchema = z.object({
  requests: z.number().int(),
  per_seconds: z.number().int(),
  burst: z.number().int().optional(),
});

export const hostPermissionBudgetSchema = z.object({
  host_permission_id: z.string(),
  declared: rateLimitSchema.nullable(),
  lowered: z
    .object({ requests: z.number().int(), per_seconds: z.number().int() })
    .nullable(),
});
export type HostPermissionBudget = z.infer<typeof hostPermissionBudgetSchema>;

export const retryResponseSchema = z.object({
  record_ids: z.array(z.uuid()),
  /** Jobs that could not start a resync of the retried records. */
  job_errors: z
    .array(z.object({ job_id: z.uuid(), message: z.string() }))
    .default([]),
});

export const previewResponseSchema = z.looseObject({
  payload: z.unknown().optional(),
  issues: z.array(z.string()).optional(),
});
export type ExportPreview = z.infer<typeof previewResponseSchema>;

/** The manifest declarations the extension page reads. */
export const manifestExportsSchema = z.looseObject({
  server: z
    .object({
      connections: z
        .array(
          z.object({
            id: z.string(),
            title: z.string(),
            grant_types: z.array(z.enum(connectionGrantTypes)),
          }),
        )
        .default([]),
      exports: z
        .array(
          z.object({
            id: z.string(),
            title: z.string(),
            connection: z.string().nullable().optional(),
            preview_command: z.string().nullable().optional(),
          }),
        )
        .default([]),
    })
    .nullable()
    .optional(),
  ui: z
    .array(
      z.object({
        id: z.string(),
        outlet: z.string().nullable().optional(),
      }),
    )
    .default([]),
});
export type ManifestExports = z.infer<typeof manifestExportsSchema>;
