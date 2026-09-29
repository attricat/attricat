export const auditPageSize = 50;
export const auditTableColumnCount = 6;
export const auditDrawerWidth = 480;
export const auditExtensionContextVersion = 1;
export const auditEventPanelOutlet = 'audit_event_panel';
export const executorTypes = { agent: 'agent', human: 'human' } as const;
export const auditOutcomeSuccess = 'success';
export const auditDateTimeFormat = {
  dateStyle: 'medium',
  timeStyle: 'medium',
} as const;
export const auditEventTitleId = (eventId: string) => `audit-event-${eventId}`;
