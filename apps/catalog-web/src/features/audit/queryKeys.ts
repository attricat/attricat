import type { AuditEventFilters } from './api';

export const auditQueryKeys = {
  all: () => ['audit-events'] as const,
  events: (filters: AuditEventFilters) =>
    [...auditQueryKeys.all(), filters] as const,
} as const;
