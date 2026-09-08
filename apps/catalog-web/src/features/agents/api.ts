import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
import {
  conversationSchema,
  messageSchema,
  runResponseSchema,
  runSchema,
  scheduleSchema,
  toolCallSchema,
} from './schemas';

const json = (method: string, body?: unknown): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: body === undefined ? undefined : JSON.stringify(body),
});
const uuidPathParam = (id: string) => encodeURIComponent(z.uuid().parse(id));
const queryParam = (value: string) => encodeURIComponent(value);

export const listConversations = () =>
  request('/api/agent/conversations', z.array(conversationSchema));
export const getConversation = (id: string) =>
  request(`/api/agent/conversations/${uuidPathParam(id)}`, conversationSchema);
export const createConversation = (title = '') =>
  request(
    '/api/agent/conversations',
    z.object({ id: z.string().uuid(), title: z.string() }),
    json('POST', { title }),
  );
export const listMessages = (id: string) =>
  request(
    `/api/agent/conversations/${uuidPathParam(id)}/messages`,
    z.array(messageSchema),
  );
export const listRuns = (id: string) =>
  request(
    `/api/agent/conversations/${uuidPathParam(id)}/runs`,
    z.array(runSchema),
  );
export const sendMessage = (
  id: string,
  content: string,
  attachmentIds: string[] = [],
) =>
  request(
    `/api/agent/conversations/${uuidPathParam(id)}/messages`,
    runResponseSchema,
    json('POST', { content, attachment_ids: attachmentIds }),
  );
export const listApprovals = (conversationId?: string) =>
  request(
    `/api/agent/approvals${conversationId ? `?conversation_id=${queryParam(conversationId)}` : ''}`,
    z.array(toolCallSchema),
  );
export const decideApproval = (id: string, approved: boolean) =>
  request(
    `/api/agent/tool-calls/${uuidPathParam(id)}/${approved ? 'approve' : 'reject'}`,
    z.unknown(),
    json('POST'),
  );
export const listSchedules = (conversationId?: string) =>
  request(
    `/api/agent/schedules${conversationId ? `?conversation_id=${queryParam(conversationId)}` : ''}`,
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
) =>
  request(
    `/api/agent/schedules/${uuidPathParam(id)}`,
    scheduleSchema,
    json('PUT', update),
  );
export const deleteSchedule = async (id: string) =>
  requestNoContent(`/api/agent/schedules/${uuidPathParam(id)}`, {
    method: 'DELETE',
  });
export const runScheduleNow = (id: string) =>
  request(
    `/api/agent/schedules/${uuidPathParam(id)}/run-now`,
    runResponseSchema,
    json('POST'),
  );
export const agentRunEventsUrl = (id: string) =>
  `/api/agent/runs/${uuidPathParam(id)}/events`;
