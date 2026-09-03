import type { AuditEventFilters } from './api';

export const auditQueryKeys = {
  events: (filters: AuditEventFilters) => ['audit-events', filters] as const,
} as const;
