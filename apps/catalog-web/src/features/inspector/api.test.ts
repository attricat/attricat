import { afterEach, describe, expect, it, vi } from 'vitest';
import { getApiHealth } from './api';

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => fetchMock.mockReset());

describe('Inspector API client', () => {
  it('checks the public API health endpoint', async () => {
    fetchMock.mockResolvedValueOnce({ ok: true });

    await expect(getApiHealth()).resolves.toBe(true);

    expect(fetchMock).toHaveBeenCalledWith('/api/health');
  });

  it('fails when the API is unavailable', async () => {
    fetchMock.mockResolvedValueOnce({ ok: false });

    await expect(getApiHealth()).rejects.toThrow('API health check failed');
  });
});
