import { z } from 'zod';
import { apiFetch } from '../auth/request';

const auditEventSchema = z.object({
  id: z.uuid(),
  occurred_at: z.string(),
  actor_user_id: z.uuid().nullable(),
  actor_display_name: z.string().nullable(),
  actor_email: z.string().nullable(),
  request_id: z.uuid(),
  correlation_id: z.uuid(),
  action: z.string(),
  authorization_scope: z.record(z.string(), z.unknown()),
  target: z.record(z.string(), z.unknown()),
  outcome: z.string(),
  metadata: z.record(z.string(), z.unknown()),
  executor_type: z.enum(['human', 'agent']),
  agent_run_id: z.uuid().nullable(),
  agent_conversation_id: z.uuid().nullable(),
  agent_tool_call_id: z.uuid().nullable(),
  agent_tool_name: z.string().nullable(),
  approval_decision: z.string().nullable(),
  approved_by_user_id: z.uuid().nullable(),
  approved_by_display_name: z.string().nullable(),
  approved_by_email: z.string().nullable(),
});

const auditEventPageSchema = z.object({
  events: z.array(auditEventSchema),
  total: z.number().int().nonnegative(),
  limit: z.number().int(),
  offset: z.number().int(),
});

export type AuditEvent = z.infer<typeof auditEventSchema>;
export type AuditEventFilters = {
  occurred_after?: string;
  occurred_before?: string;
  actor_user_id?: string;
  action_category?: string;
  target_type?: string;
  executor_type?: 'human' | 'agent';
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
  const response = await apiFetch(`/api/audit-events?${search}`);
  if (!response.ok)
    throw new Error(`Unable to load activity (${response.status})`);
  return auditEventPageSchema.parse(await response.json());
};
