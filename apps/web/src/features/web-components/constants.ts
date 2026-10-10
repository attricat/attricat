export const attricatEventNames = {
  recordUpdated: 'attricat:record-updated.v1',
  contextChanged: 'attricat:context-changed.v1',
  refreshRecord: 'attricat:refresh-record.v1',
  navigate: 'attricat:navigate.v1',
  notify: 'attricat:notify.v1',
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
