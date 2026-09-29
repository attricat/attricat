const millisecondsPerSecond = 1_000;

export const API_HEALTH_POLL_INTERVAL_MS = 5_000;
export const API_HEALTH_POLL_INTERVAL_SECONDS =
  API_HEALTH_POLL_INTERVAL_MS / millisecondsPerSecond;
export const API_RETRY_INTERVAL_MS = 1_000;
export const INSPECTOR_STATE_STORAGE_KEY = 'catalog.inspector-expanded';

export const inspectorPaneIds = {
  server: 'server',
  performance: 'performance',
  session: 'session',
} as const;
export type InspectorPaneId =
  (typeof inspectorPaneIds)[keyof typeof inspectorPaneIds];
export const inspectorPaneOrder: readonly InspectorPaneId[] = [
  inspectorPaneIds.server,
  inspectorPaneIds.performance,
  inspectorPaneIds.session,
];

export const personalApiTokensHref = '/profile#personal-api-tokens';

/** Shown in place of a query count the API did not report. */
export const unknownQueryCount = '?';
export const timingDurationFractionDigits = 2;

export const inspectorHeaderMinHeight = 36;
export const inspectorTabMinHeight = 28;
export const inspectorPanelHeight = '20vh';
export const launcherStatusDotSize = 10;
export const launcherStatusDotOffset = 3;
export const headerStatusDotSize = 8;
