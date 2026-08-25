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

  it('rejects a session response without its workspace login identifier', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ ...session, login_identifier: undefined }),
    });
    await expect(currentSession()).rejects.toThrow();
  });
});
