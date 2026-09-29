export const catalogEventNames = {
  entityUpdated: 'catalog:entity-updated.v1',
  contextChanged: 'catalog:context-changed.v1',
  refreshEntity: 'catalog:refresh-entity.v1',
  navigate: 'catalog:navigate.v1',
  notify: 'catalog:notify.v1',
} as const;

export const entityChangeHints = [
  'entity',
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
