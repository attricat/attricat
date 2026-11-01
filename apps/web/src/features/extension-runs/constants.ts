export const extensionRunsPath = '/api/extension-runs';
export const extensionRunsPagePath = '/profile/extension-runs';

/** Execution states; the extension's domain outcome is reported in progress. */
export const extensionRunStatuses = {
  queued: 'queued',
  running: 'running',
  cancelling: 'cancelling',
  cancelled: 'cancelled',
  completed: 'completed',
  failed: 'failed',
} as const;
export type ExtensionRunStatus =
  (typeof extensionRunStatuses)[keyof typeof extensionRunStatuses];
export const activeExtensionRunStatuses: readonly ExtensionRunStatus[] = [
  extensionRunStatuses.queued,
  extensionRunStatuses.running,
  extensionRunStatuses.cancelling,
];

export const extensionRunFailures = [
  'access_revoked',
  'extension_failed',
] as const;

/** Poll only while a watched run is active. */
export const extensionRunPollMilliseconds = 2_000;
/** Bound tracking to recently started runs in this tab. */
export const maximumWatchedExtensionRuns = 20;

export const runSkeletonCount = 2;
export const runSkeletonHeight = 136;
export const runDetailSkeletonHeight = 240;
/** Whose runs a request covers: all the caller can see, or only their own. */
export const extensionRunScopes = { visible: 'visible', own: 'own' } as const;

/** What a run reaches: its selected records, or one blueprint revision. */
export const extensionRunSelectionScopes = {
  selection: 'selection',
  blueprint: 'blueprint',
} as const;
