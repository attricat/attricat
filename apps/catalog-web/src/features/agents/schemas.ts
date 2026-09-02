import { z } from 'zod';

const dateTime = z.string().datetime({ offset: true });
const id = z.string().uuid();

export const conversationSchema = z.object({
  id,
  workspace_id: id,
  created_by_user_id: id.nullable(),
  title: z.string(),
  created_at: dateTime,
  updated_at: dateTime,
  archived_at: dateTime.nullable(),
});
export const messageAttachmentSchema = z.object({
  id,
  filename: z.string(),
  mime_type: z.string(),
  byte_size: z.number().int().nonnegative(),
  status: z.string(),
});
export const messageSchema = z.object({
  id,
  conversation_id: id,
  run_id: id.nullable(),
  sequence: z.number(),
  role: z.string(),
  content: z.unknown(),
  created_at: dateTime,
  attachments: z.array(messageAttachmentSchema).default([]),
});
export const runSchema = z.object({
  id,
  conversation_id: id,
  schedule_id: id.nullable(),
  origin: z.string(),
  status: z.string(),
  provider_base_url: z.string(),
  model: z.string(),
  started_at: dateTime.nullable(),
  finished_at: dateTime.nullable(),
  error_code: z.string().nullable(),
  error_message: z.string().nullable(),
  created_at: dateTime,
});
export const toolCallSchema = z.object({
  id,
  run_id: id,
  sequence: z.number(),
  provider_call_id: z.string().nullable(),
  tool_name: z.string(),
  arguments: z.unknown(),
  change_summary: z.string().nullable(),
  result: z.unknown().nullable(),
  error: z.unknown().nullable(),
  state: z.string(),
  decided_by_user_id: id.nullable(),
  decided_at: dateTime.nullable(),
  created_at: dateTime,
  completed_at: dateTime.nullable(),
});
export const scheduleSchema = z.object({
  id,
  conversation_id: id,
  initiated_by_user_id: id.nullable(),
  cron_expression: z.string(),
  timezone: z.string(),
  enabled: z.boolean(),
  next_run_at: dateTime.nullable(),
  last_run_at: dateTime.nullable(),
  created_at: dateTime,
  updated_at: dateTime,
});
export const runResponseSchema = z.object({ id, status: z.string() });

export type Conversation = z.infer<typeof conversationSchema>;
export type ConversationMessage = z.infer<typeof messageSchema>;
export type ConversationMessageAttachment = z.infer<
  typeof messageAttachmentSchema
>;
export type AgentRun = z.infer<typeof runSchema>;
export type AgentToolCall = z.infer<typeof toolCallSchema>;
export type AgentSchedule = z.infer<typeof scheduleSchema>;
