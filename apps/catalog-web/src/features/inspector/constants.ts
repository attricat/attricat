const millisecondsPerSecond = 1_000;

export const API_HEALTH_POLL_INTERVAL_MS = 5_000;
export const API_HEALTH_POLL_INTERVAL_SECONDS =
  API_HEALTH_POLL_INTERVAL_MS / millisecondsPerSecond;
export const API_RETRY_INTERVAL_MS = 1_000;
export const INSPECTOR_STATE_STORAGE_KEY = 'catalog.inspector-expanded';
/** Set to `true` to load the Inspector in builds without devtools. */
export const INSPECTOR_ENABLED_STORAGE_KEY = 'catalog.inspector-enabled';

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

export const personalApiTokensHref = '/profile/personal-access-tokens';

/** Shown in place of a query count the API did not report. */
export const unknownQueryCount = '?';
export const timingDurationFractionDigits = 2;
/** Git's conventional abbreviated commit length. */
export const shortCommitLength = 7;

export const inspectorHeaderMinHeight = 36;
export const inspectorTabMinHeight = 28;
export const inspectorPanelHeight = '20vh';
export const launcherStatusDotSize = 10;
export const launcherStatusDotOffset = 3;
export const headerStatusDotSize = 8;
/** Width of the bottom-edge strip that reveals the Inspector launcher. */
export const launcherRevealZoneWidth = 200;
/** Hover target height for precise pointers; touch gets a larger tap target. */
export const launcherRevealZoneHeight = 10;
export const launcherRevealZoneTouchHeight = 24;
/** Neutral grabber pill hinting at the hidden launcher. */
export const launcherHandleWidth = 64;
export const launcherHandleHeight = 4;
export const launcherHandleOffset = 3;
/** Shimmer across the handle while the API is checking or restarting. */
export const launcherLoadingAnimationName = 'inspector-launcher-loading';
export const launcherLoadingAnimationDuration = '1.2s';
/** A touch-revealed launcher hides again when left untouched. */
export const LAUNCHER_TOUCH_REVEAL_MS = 4_000;
