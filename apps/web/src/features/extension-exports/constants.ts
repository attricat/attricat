/** Per-record export statuses, in the order lists and filters show them. */
export const exportStatuses = [
  'pending',
  'retrying',
  'uncertain',
  'rejected',
  'synced',
  'withdrawn',
] as const;
export type ExportStatus = (typeof exportStatuses)[number];

/** Statuses an operator should look at; the record page flags them. */
export const exportProblemStatuses: readonly ExportStatus[] = [
  'rejected',
  'uncertain',
  'retrying',
];

/** Statuses an operator can retry. */
export const retryableExportStatuses: readonly ExportStatus[] = [
  'rejected',
  'uncertain',
  'retrying',
  'pending',
];

export const connectionGrantTypes = [
  'client_credentials',
  'refresh_token',
  'device_code',
  'authorization_code',
] as const;
export type ConnectionGrantType = (typeof connectionGrantTypes)[number];

export const exportStatusPath = '/api/export-status';
export const recordExportStatusPath = (recordId: string) =>
  `/api/v1/records/${encodeURIComponent(recordId)}/export-status`;
export const extensionPath = (extensionId: string) =>
  `/api/extensions/${encodeURIComponent(extensionId)}`;

/** Rows of one problem list page on the extension page. */
export const exportStatusPageSize = 25;
/** Device authorizations are polled no faster than the provider allows. */
export const minimumDevicePollMilliseconds = 5_000;

/** The host outlet where an export extension edits its destination mappings. */
export const exportMappingExtensionOutlet = 'export_mapping';
