import { z } from 'zod';
import { request } from '../../api/request';
import {
  conversationCreateResponseSchema,
  conversationSchema,
  conversationSearchPageSchema,
  messageSchema,
  runResponseSchema,
  runSchema,
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
export const searchConversations = (query: string, cursor?: string) => {
  const params = new URLSearchParams({ q: query });
  if (cursor) params.set('cursor', cursor);
  return request(
    `/api/agent/conversations/search?${params}`,
    conversationSearchPageSchema,
  );
};
export const getConversation = (id: string) =>
  request(`/api/agent/conversations/${uuidPathParam(id)}`, conversationSchema);
export const createConversation = (
  title = '',
  entity?: { entity_id: string; context_id?: string | null },
) =>
  request(
    '/api/agent/conversations',
    conversationCreateResponseSchema,
    json('POST', { title, ...entity }),
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
export const agentRunEventsUrl = (id: string) =>
  `/api/agent/runs/${uuidPathParam(id)}/events`;
