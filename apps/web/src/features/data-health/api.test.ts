import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  getDataHealthBlueprints,
  getDataHealthSummary,
  refreshDataHealth,
} from './api';
import '../../i18n';

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => {
  fetchMock.mockReset();
});

const respond = (body: unknown) => {
  fetchMock.mockResolvedValue({ ok: true, json: () => Promise.resolve(body) });
};

describe('data health API client', () => {
  it('requests threshold-specific health sections', async () => {
    respond({
      active_records: 3,
      record_blueprints: 1,
      contexts: 1,
      outdated_records: 1,
      stale_records: 2,
      deleted_relationship_targets: 0,
    });
    await getDataHealthSummary(180);
    expect(fetchMock).toHaveBeenLastCalledWith(
      '/api/data-health/summary?stale_after_days=180',
    );

    respond([]);
    await getDataHealthBlueprints(30);
    expect(fetchMock).toHaveBeenLastCalledWith(
      '/api/data-health/blueprints?stale_after_days=30',
    );
  });

  it('rejects malformed API responses', async () => {
    respond({ active_records: 'three' });
    await expect(getDataHealthSummary(90)).rejects.toThrow(
      'Invalid API response',
    );
  });

  it('clears the server health cache before a manual refresh', async () => {
    fetchMock.mockResolvedValue({ ok: true });
    await refreshDataHealth();
    expect(fetchMock).toHaveBeenLastCalledWith('/api/data-health/refresh', {
      method: 'POST',
    });
  });
});
