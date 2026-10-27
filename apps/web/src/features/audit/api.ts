import { request } from '../../api/request';
import { auditEventPageSchema } from './schemas';
import type { ExecutorType } from './constants';
export type { AuditEvent } from './schemas';

export type AuditEventFilters = {
  occurred_after?: string;
  occurred_before?: string;
  actor_user_id?: string;
  action_category?: string;
  target_type?: string;
  executor_type?: ExecutorType;
  agent_run_id?: string;
  agent_tool_call_id?: string;
  limit?: number;
  offset?: number;
};

export const listAuditEvents = async (filters: AuditEventFilters) => {
  const search = new URLSearchParams();
  Object.entries(filters).forEach(([key, value]) => {
    if (value !== undefined && value !== '') search.set(key, String(value));
  });
  return request(`/api/audit-events?${search}`, auditEventPageSchema);
};
