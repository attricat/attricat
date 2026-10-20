import { afterEach, describe, expect, it, vi } from 'vitest';
import { loadRecordPublications } from './recordPublicationsLoader';

const first = '123e4567-e89b-12d3-a456-426614174001';
const second = '123e4567-e89b-12d3-a456-426614174002';
const hidden = '123e4567-e89b-12d3-a456-426614174003';
const publication = {
  context_id: '223e4567-e89b-12d3-a456-426614174000',
  context_code: 'web',
  status: 'published',
  published_at: '2026-10-20T10:00:00Z',
  published_by_user_id: null,
};

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => {
  fetchMock.mockReset();
});

const json = (body: unknown, status = 200) => ({
  ok: status < 400,
  status,
  json: () => Promise.resolve(body),
});

describe('loadRecordPublications', () => {
  it('looks up records requested together in one request', async () => {
    fetchMock.mockResolvedValueOnce(
      json({
        items: [
          { record_id: first, publications: [publication] },
          { record_id: second, publications: [] },
        ],
      }),
    );
    const [firstStatuses, secondStatuses, repeated] = await Promise.all([
      loadRecordPublications(first),
      loadRecordPublications(second),
      loadRecordPublications(first),
    ]);
    expect(firstStatuses).toEqual([publication]);
    expect(repeated).toEqual([publication]);
    expect(secondStatuses).toEqual([]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/v1/records/publications',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ record_ids: [first, second] }),
      }),
    );
  });

  it('fails a left-out record as its own lookup does', async () => {
    fetchMock
      .mockResolvedValueOnce(json({ items: [] }))
      .mockResolvedValueOnce(
        json(
          { error: { code: 'not_found', message: 'record not found' } },
          404,
        ),
      );
    await expect(loadRecordPublications(hidden)).rejects.toMatchObject({
      status: 404,
    });
    expect(fetchMock.mock.lastCall?.[0]).toBe(
      `/api/v1/records/${hidden}/publications`,
    );
  });
});
