import { z } from 'zod';
import { apiFetch } from '../auth/request';
import {
  conversationSchema,
  messageSchema,
  runResponseSchema,
  runSchema,
  scheduleSchema,
  toolCallSchema,
} from './schemas';

const request = async <T>(
  path: string,
  schema: z.ZodType<T>,
  init?: RequestInit,
): Promise<T> => {
  const response = await apiFetch(path, init);
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  const parsed = schema.safeParse(
    response.status === 204 ? undefined : await response.json(),
  );
  if (!parsed.success)
    throw new Error(`Invalid API response: ${z.prettifyError(parsed.error)}`);
  return parsed.data;
};
const json = (method: string, body?: unknown): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: body === undefined ? undefined : JSON.stringify(body),
});

export const listConversations = () =>
  request('/api/agent/conversations', z.array(conversationSchema));
export const getConversation = (id: string) =>
  request(`/api/agent/conversations/${id}`, conversationSchema);
export const createConversation = (title = '') =>
  request(
    '/api/agent/conversations',
    z.object({ id: z.string().uuid(), title: z.string() }),
    json('POST', { title }),
  );
export const listMessages = (id: string) =>
  request(`/api/agent/conversations/${id}/messages`, z.array(messageSchema));
export const listRuns = (id: string) =>
  request(`/api/agent/conversations/${id}/runs`, z.array(runSchema));
export const sendMessage = (
  id: string,
  content: string,
  attachmentIds: string[] = [],
) =>
  request(
    `/api/agent/conversations/${id}/messages`,
    runResponseSchema,
    json('POST', { content, attachment_ids: attachmentIds }),
  );
export const listApprovals = (conversationId?: string) =>
  request(
    `/api/agent/approvals${conversationId ? `?conversation_id=${conversationId}` : ''}`,
    z.array(toolCallSchema),
  );
export const decideApproval = (id: string, approved: boolean) =>
  request(
    `/api/agent/tool-calls/${id}/${approved ? 'approve' : 'reject'}`,
    z.unknown(),
    json('POST'),
  );
export const listSchedules = (conversationId?: string) =>
  request(
    `/api/agent/schedules${conversationId ? `?conversation_id=${conversationId}` : ''}`,
    z.array(scheduleSchema),
  );
export const createSchedule = (
  conversationId: string,
  cronExpression: string,
) =>
  request(
    '/api/agent/schedules',
    scheduleSchema,
    json('POST', {
      conversation_id: conversationId,
      cron_expression: cronExpression,
    }),
  );
export const updateSchedule = (
  id: string,
  update: { cron_expression?: string; enabled?: boolean },
) => request(`/api/agent/schedules/${id}`, scheduleSchema, json('PUT', update));
export const deleteSchedule = async (id: string) => {
  const response = await apiFetch(`/api/agent/schedules/${id}`, {
    method: 'DELETE',
  });
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
};
export const runScheduleNow = (id: string) =>
  request(
    `/api/agent/schedules/${id}/run-now`,
    runResponseSchema,
    json('POST'),
  );
