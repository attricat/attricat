import { afterEach, describe, expect, it, vi } from 'vitest';
import { currentSession, login } from './api';

const session = {
  user_id: '123e4567-e89b-12d3-a456-426614174000',
  display_name: null,
  email: 'user@example.test',
  workspace_id: '223e4567-e89b-12d3-a456-426614174000',
  login_identifier: 'example.local',
};
const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);
vi.stubGlobal('document', { cookie: '' });

afterEach(() => fetchMock.mockReset());

describe('session API client', () => {
  it('returns the active workspace login identifier from login and current-session responses', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(session),
    });
    await expect(
      login('example.local', 'user@example.test', 'password'),
    ).resolves.toMatchObject({
      login_identifier: 'example.local',
    });
    await expect(currentSession()).resolves.toMatchObject({
      login_identifier: 'example.local',
    });
  });

  it('returns null only when the session endpoint reports unauthenticated', async () => {
    fetchMock.mockResolvedValue({ ok: false, status: 401 });

    await expect(currentSession()).resolves.toBeNull();
  });

  it('throws a useful error for non-authentication session failures', async () => {
    fetchMock.mockResolvedValue({
      ok: false,
      status: 503,
      statusText: 'Service Unavailable',
    });

    await expect(currentSession()).rejects.toThrow(
      'Unable to check the current session (HTTP 503 Service Unavailable)',
    );

    fetchMock.mockResolvedValue({
      ok: false,
      status: 403,
      statusText: 'Forbidden',
    });

    await expect(currentSession()).rejects.toThrow(
      'Unable to check the current session (HTTP 403 Forbidden)',
    );
  });

  it('propagates network failures rather than treating them as unauthenticated', async () => {
    fetchMock.mockRejectedValue(new TypeError('Network request failed'));

    await expect(currentSession()).rejects.toThrow('Network request failed');
  });

  it('rejects a session response without its workspace login identifier', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ ...session, login_identifier: undefined }),
    });
    await expect(currentSession()).rejects.toThrow();
  });
});
