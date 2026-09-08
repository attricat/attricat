import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import { ApiRequestError, request, requestNoContent } from './request';

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => fetchMock.mockReset());

describe('API request helper', () => {
  it('preserves API error status, code, and message', async () => {
    fetchMock.mockResolvedValue({
      ok: false,
      status: 422,
      json: () =>
        Promise.resolve({
          error: { code: 'validation_failed', message: 'Title is required' },
        }),
    });

    await expect(
      request('/api/example', z.object({ id: z.string() })),
    ).rejects.toMatchObject({
      name: 'ApiRequestError',
      status: 422,
      code: 'validation_failed',
      message: 'Title is required',
    } satisfies Partial<ApiRequestError>);
  });

  it('validates successful JSON responses with the provided schema', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve({ id: 1 }),
    });

    await expect(
      request('/api/example', z.object({ id: z.string() })),
    ).rejects.toThrow('Invalid API response');
  });

  it('supports successful no-content requests without reading a response body', async () => {
    fetchMock.mockResolvedValue({ ok: true, status: 204 });

    await expect(
      requestNoContent('/api/example', { method: 'DELETE' }),
    ).resolves.toBeUndefined();
    expect(fetchMock).toHaveBeenCalledWith('/api/example', {
      method: 'DELETE',
    });
  });
});
