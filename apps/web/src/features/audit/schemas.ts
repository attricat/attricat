import { z } from 'zod';
import { executorTypes } from './constants';

const auditEventSchema = z.object({
  id: z.uuid(),
  occurred_at: z.string(),
  actor_user_id: z.uuid().nullable(),
  actor_display_name: z.string().nullable(),
  actor_email: z.string().nullable(),
  actor_avatar_file_id: z.uuid().nullable().default(null),
  request_id: z.uuid(),
  correlation_id: z.uuid(),
  action: z.string(),
  authorization_scope: z.record(z.string(), z.unknown()),
  target: z.record(z.string(), z.unknown()),
  outcome: z.string(),
  metadata: z.record(z.string(), z.unknown()),
  executor_type: z.enum(executorTypes),
  agent_run_id: z.uuid().nullable(),
  agent_conversation_id: z.uuid().nullable(),
  agent_tool_call_id: z.uuid().nullable(),
  agent_tool_name: z.string().nullable(),
  approval_decision: z.string().nullable(),
  approved_by_user_id: z.uuid().nullable(),
  approved_by_display_name: z.string().nullable(),
  approved_by_email: z.string().nullable(),
  approved_by_avatar_file_id: z.uuid().nullable().default(null),
});

export const auditEventPageSchema = z.object({
  events: z.array(auditEventSchema),
  total: z.number().int().nonnegative(),
  limit: z.number().int(),
  offset: z.number().int(),
});

export type AuditEvent = z.infer<typeof auditEventSchema>;
