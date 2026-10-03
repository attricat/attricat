export const extensionRunsPath = '/api/extension-runs';
export const extensionRunsPagePath = '/profile/extension-runs';

/** Execution states; the extension's domain outcome is reported in progress. */
export const extensionRunStatuses = [
  'queued',
  'running',
  'cancelling',
  'cancelled',
  'completed',
  'failed',
] as const;
export type ExtensionRunStatus = (typeof extensionRunStatuses)[number];
export const activeExtensionRunStatuses: readonly ExtensionRunStatus[] = [
  'queued',
  'running',
  'cancelling',
];

export const extensionRunFailures = [
  'access_revoked',
  'extension_failed',
] as const;

/** Poll only while a watched run is active. */
export const extensionRunPollMilliseconds = 2_000;
/** Bound tracking to recently started runs in this tab. */
export const maximumWatchedExtensionRuns = 20;
export const extensionRunIdempotencyPrefix = 'run-';

export const runSkeletonCount = 2;
export const runSkeletonHeight = 136;
export const runDetailSkeletonHeight = 240;
