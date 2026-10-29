import { z } from 'zod';
import {
  agentRunStatuses,
  bulkProgressStates,
  titleSources,
} from './constants';

const dateTime = z.string().datetime({ offset: true });
const id = z.string().uuid();

export const conversationSchema = z.object({
  id,
  workspace_id: id,
  created_by_user_id: id.nullable(),
  title: z.string(),
  title_source: z.enum(titleSources),
  record_id: id.nullable(),
  context_id: id.nullable(),
  created_at: dateTime,
  updated_at: dateTime,
  archived_at: dateTime.nullable(),
  /** Who has the agent apply this record's edits without asking. */
  auto_approve_user_id: id.nullable().default(null),
  auto_approve_enabled_at: dateTime.nullable().default(null),
});
export const conversationDetailSchema = conversationSchema.extend({
  read_only: z.boolean(),
});
const messageAttachmentSchema = z.object({
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
  origin: z.string(),
  status: z.enum(agentRunStatuses),
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
export const conversationSearchPageSchema = z.object({
  items: z.array(conversationSchema),
  next_cursor: z.string().nullable(),
});
export const conversationCreateResponseSchema = z.object({
  id,
  title: z.string(),
});
export const runResponseSchema = z.object({ id, status: z.string() });

export type Conversation = z.infer<typeof conversationSchema>;
export type ConversationMessage = z.infer<typeof messageSchema>;
export type AgentRun = z.infer<typeof runSchema>;
export type AgentToolCall = z.infer<typeof toolCallSchema>;

const bulkEditNamedCodeSchema = z.object({
  code: z.string(),
  name: z.string(),
});

/** The resolved proposal a bulk edit's approval covers. */
export const bulkEditProposalSchema = z.object({
  blueprint: bulkEditNamedCodeSchema.nullable(),
  attributes: z.array(bulkEditNamedCodeSchema),
  records: z.array(
    z.object({
      record_id: id,
      blueprint_code: z.string(),
      label: z.string().nullable(),
      values: z.array(
        z.object({
          attribute_code: z.string(),
          context_id: id.nullable(),
          value: z.unknown(),
          previous: z.unknown(),
        }),
      ),
    }),
  ),
});

export const bulkEditResultSchema = z.object({
  applied: z.array(id),
  failed: z.array(
    z.object({
      record_id: id,
      code: z.string(),
      message: z.string(),
      details: z.unknown().optional(),
    }),
  ),
  not_run: z.array(id),
});

export const bulkProgressSchema = z.object({
  run_id: id,
  payload: z.object({
    tool_call_id: id,
    record_id: id,
    state: z.enum(bulkProgressStates),
    applied: z.number().int().nonnegative(),
    total: z.number().int().positive(),
  }),
});

export type BulkEditProposal = z.infer<typeof bulkEditProposalSchema>;
export type BulkEditResult = z.infer<typeof bulkEditResultSchema>;
export type BulkProgress = z.infer<typeof bulkProgressSchema>['payload'] & {
  run_id: string;
};
