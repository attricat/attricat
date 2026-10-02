import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  agentRunEventsUrl,
  createConversation,
  decideApproval,
  getConversation,
  listApprovals,
  listMessages,
  listRuns,
  sendMessage,
  searchConversations,
  updateConversationTitle,
} from './api';
import '../../i18n';

const id = '123e4567-e89b-12d3-a456-426614174000';
const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => fetchMock.mockReset());

describe('agent API client', () => {
  it.each([getConversation, listMessages, listRuns, listApprovals])(
    'forwards cancellation to conversation reads (%#)',
    async (read) => {
      const controller = new AbortController();
      fetchMock.mockImplementation(
        (_path, init: RequestInit) =>
          new Promise((_resolve, reject) => {
            init.signal?.addEventListener('abort', () =>
              reject(new DOMException('Aborted', 'AbortError')),
            );
          }),
      );
      const result = read(id, controller.signal);
      controller.abort();
      await expect(result).rejects.toMatchObject({ name: 'AbortError' });
      expect(fetchMock.mock.calls[0][1].signal).toBe(controller.signal);
    },
  );

  it('sends conversations and messages with credentials and CSRF protection', async () => {
    fetchMock
      .mockResolvedValueOnce({
        ok: true,
        status: 201,
        json: () => Promise.resolve({ id, title: 'Review products' }),
      })
      .mockResolvedValueOnce({
        ok: true,
        status: 202,
        json: () => Promise.resolve({ id, status: 'queued' }),
      });

    await expect(createConversation('Review products')).resolves.toEqual({
      id,
      title: 'Review products',
    });
    await expect(sendMessage(id, 'List products', [id])).resolves.toEqual({
      id,
      status: 'queued',
    });

    const [conversationPath, conversationInit] = fetchMock.mock.calls[0];
    expect(conversationPath).toBe('/api/agent/conversations');
    expect(JSON.parse(conversationInit.body)).toEqual({
      title: 'Review products',
    });
    expect(fetchMock.mock.calls[1][0]).toBe(
      `/api/agent/conversations/${id}/messages`,
    );
    expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toEqual({
      content: 'List products',
      attachment_ids: [id],
    });
  });

  it('binds a new conversation to the entity and selected context', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      status: 201,
      json: () => Promise.resolve({ id, title: 'Entity' }),
    });
    await createConversation('Entity', { entity_id: id, context_id: id });
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({
      title: 'Entity',
      entity_id: id,
      context_id: id,
    });
  });

  it('sends a manual conversation title update', async () => {
    const conversation = {
      id,
      workspace_id: id,
      created_by_user_id: null,
      title: 'Pricing review',
      title_source: 'manual',
      entity_id: null,
      context_id: null,
      created_at: '2026-09-28T00:00:00Z',
      updated_at: '2026-09-28T00:00:00Z',
      archived_at: null,
    };
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(conversation),
    });
    await expect(
      updateConversationTitle(id, 'Pricing review'),
    ).resolves.toEqual(conversation);
    expect(fetchMock.mock.calls[0][0]).toBe(`/api/agent/conversations/${id}`);
    expect(fetchMock.mock.calls[0][1]).toEqual({
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ title: 'Pricing review' }),
    });
  });

  it('encodes search text and pagination cursor', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ items: [], next_cursor: null }),
    });
    await searchConversations('price & specs', 'date|id');
    expect(fetchMock.mock.calls[0][0]).toBe(
      '/api/agent/conversations/search?q=price+%26+specs&cursor=date%7Cid',
    );
  });

  it('rejects malformed UUID path IDs before issuing requests', async () => {
    const malformedId = 'not-a-uuid';

    expect(() => getConversation(malformedId)).toThrow('Invalid UUID');
    expect(() => listMessages(malformedId)).toThrow('Invalid UUID');
    expect(() => listRuns(malformedId)).toThrow('Invalid UUID');
    expect(() => sendMessage(malformedId, 'Hello')).toThrow('Invalid UUID');
    expect(() => decideApproval(malformedId, true)).toThrow('Invalid UUID');
    expect(() => agentRunEventsUrl(malformedId)).toThrow('Invalid UUID');
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('encodes dynamic query values', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve([]),
    });

    await listApprovals('space & slash/');

    expect(fetchMock.mock.calls.map(([path]) => path)).toEqual([
      '/api/agent/approvals?conversation_id=space%20%26%20slash%2F',
    ]);
  });

  it('rejects unsuccessful or invalid responses', async () => {
    fetchMock.mockResolvedValueOnce({ ok: false, status: 503 });
    await expect(createConversation('Unavailable')).rejects.toThrow(
      'Request failed (503)',
    );

    fetchMock.mockResolvedValueOnce({
      ok: true,
      status: 202,
      json: () => Promise.resolve({ id: 'not-a-uuid', status: 'queued' }),
    });
    await expect(sendMessage(id, 'Hello')).rejects.toThrow(
      'Invalid API response',
    );
  });
});
