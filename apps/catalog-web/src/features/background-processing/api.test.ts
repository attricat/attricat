import { afterEach, describe, expect, it, vi } from 'vitest';
import { getBackgroundProcessingStatus } from './api';
import { backgroundProcessingQueryKeys } from './query-keys';

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);
afterEach(() => {
  fetchMock.mockReset();
});

describe('background processing API', () => {
  it('requests workspace status through the JSON client', async () => {
    fetchMock.mockResolvedValue({ ok: true, json: async () => [] });
    expect(await getBackgroundProcessingStatus()).toEqual([]);
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/data-health/background-processing',
    );
  });
  it('rejects malformed queue counts', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => [
        {
          kind: 'rule_run.v1',
          queued: -1,
          running: 0,
          failed: 0,
          expired_leases: 0,
          oldest_due_seconds: null,
        },
      ],
    });
    await expect(getBackgroundProcessingStatus()).rejects.toThrow(
      'Invalid API response',
    );
  });
  it('keeps workspace cache keys distinct', () => {
    expect(backgroundProcessingQueryKeys.status('one')).not.toEqual(
      backgroundProcessingQueryKeys.status('two'),
    );
  });
});
