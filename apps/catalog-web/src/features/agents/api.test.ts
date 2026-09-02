import { afterEach, describe, expect, it, vi } from 'vitest';
import { createConversation, sendMessage } from './api';

const id = '123e4567-e89b-12d3-a456-426614174000';
const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => fetchMock.mockReset());

describe('agent API client', () => {
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
    expect(JSON.parse(conversationInit.body)).toEqual({ title: 'Review products' });
    expect(fetchMock.mock.calls[1][0]).toBe(
      `/api/agent/conversations/${id}/messages`,
    );
    expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toEqual({
      content: 'List products',
      attachment_ids: [id],
    });
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
    await expect(sendMessage(id, 'Hello')).rejects.toThrow('Invalid API response');
  });
});
