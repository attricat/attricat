import { z } from 'zod';
import { request } from '../../api/request';
import {
  conversationCreateResponseSchema,
  conversationDetailSchema,
  conversationSchema,
  conversationSearchPageSchema,
  messageSchema,
  runResponseSchema,
  runSchema,
  toolCallSchema,
} from './schemas';
import { uuidSegment } from '../../api/pathSegments';

const json = (method: string, body?: unknown): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: body === undefined ? undefined : JSON.stringify(body),
});
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
export const getConversation = (id: string, signal?: AbortSignal) =>
  request(
    `/api/agent/conversations/${uuidSegment(id)}`,
    conversationDetailSchema,
    { signal },
  );
export const updateConversationTitle = (id: string, title: string) =>
  request(
    `/api/agent/conversations/${uuidSegment(id)}`,
    conversationSchema,
    json('PATCH', { title }),
  );
export const createConversation = (
  title = '',
  record?: { record_id: string; context_id?: string | null },
) =>
  request(
    '/api/agent/conversations',
    conversationCreateResponseSchema,
    json('POST', { title, ...record }),
  );
export const listMessages = (id: string, signal?: AbortSignal) =>
  request(
    `/api/agent/conversations/${uuidSegment(id)}/messages`,
    z.array(messageSchema),
    { signal },
  );
export const listRuns = (id: string, signal?: AbortSignal) =>
  request(
    `/api/agent/conversations/${uuidSegment(id)}/runs`,
    z.array(runSchema),
    { signal },
  );
export const sendMessage = (
  id: string,
  content: string,
  attachmentIds: string[] = [],
) =>
  request(
    `/api/agent/conversations/${uuidSegment(id)}/messages`,
    runResponseSchema,
    json('POST', { content, attachment_ids: attachmentIds }),
  );
export const listApprovals = (conversationId?: string, signal?: AbortSignal) =>
  request(
    `/api/agent/approvals${conversationId ? `?conversation_id=${queryParam(conversationId)}` : ''}`,
    z.array(toolCallSchema),
    { signal },
  );
export const decideApproval = (id: string, approved: boolean) =>
  request(
    `/api/agent/tool-calls/${uuidSegment(id)}/${approved ? 'approve' : 'reject'}`,
    z.unknown(),
    json('POST'),
  );
export const agentRunEventsUrl = (id: string) =>
  `/api/agent/runs/${uuidSegment(id)}/events`;
