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

/** The status the extension page lists first: exports the destination refused. */
export const initialExportStatusFilter: ExportStatus = 'rejected';

/** Statuses an operator can retry. */
export const retryableExportStatuses: readonly ExportStatus[] = [
  'rejected',
  'uncertain',
  'retrying',
  'pending',
];

export const connectionGrantTypes = {
  clientCredentials: 'client_credentials',
  refreshToken: 'refresh_token',
  deviceCode: 'device_code',
  authorizationCode: 'authorization_code',
} as const;
export type ConnectionGrantType =
  (typeof connectionGrantTypes)[keyof typeof connectionGrantTypes];

/** Server-side steps a connection can be asked to run. */
export const connectionActions = {
  deviceAuthorization: 'device-authorization',
  devicePoll: 'device-authorization/poll',
  test: 'test',
} as const;
export type ConnectionAction =
  (typeof connectionActions)[keyof typeof connectionActions];

export const connectionStatuses = {
  pendingAuthorization: 'pending_authorization',
  active: 'active',
  failed: 'failed',
  revoked: 'revoked',
} as const;

/** How the public OAuth callback reports a browser authorization. */
export const browserAuthorizationOutcomes = [
  'connected',
  'failed',
  'expired',
  'signed_out',
] as const;
export type BrowserAuthorizationOutcome =
  (typeof browserAuthorizationOutcomes)[number];

export const exportStatusPath = '/api/export-status';
export const recordExportStatusPath = (recordId: string) =>
  `/api/v1/records/${encodeURIComponent(recordId)}/export-status`;

/** Longest connection code a returning OAuth callback may name. */
export const maximumConnectionCodeLength = 128;
export const exportFilterMinWidth = 200;

/** Rows of one problem list page on the extension page. */
export const exportStatusPageSize = 25;
