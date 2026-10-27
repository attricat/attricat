export const auditPageSize = 50;
export const auditTableColumnCount = 6;
export const auditDrawerWidth = 480;
export const executorTypes = { agent: 'agent', human: 'human' } as const;
export type ExecutorType = (typeof executorTypes)[keyof typeof executorTypes];
export const auditOutcomeSuccess = 'success';
export const auditEventTitleId = (eventId: string) => `audit-event-${eventId}`;
