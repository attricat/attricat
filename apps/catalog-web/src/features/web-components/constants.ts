export const catalogEventNames = {
  recordUpdated: 'catalog:record-updated.v1',
  contextChanged: 'catalog:context-changed.v1',
  refreshRecord: 'catalog:refresh-record.v1',
  navigate: 'catalog:navigate.v1',
  notify: 'catalog:notify.v1',
} as const;

export const recordChangeHints = [
  'record',
  'attribute_values',
  'relationships',
  'blueprint',
] as const;

export const notificationSeverities = [
  'success',
  'info',
  'warning',
  'error',
] as const;
export const defaultNotificationSeverity = 'info';

/** Upper bound for component- or extension-supplied notification text. */
export const maximumNotificationMessageLength = 512;
