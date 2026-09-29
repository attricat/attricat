import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiRequestError } from '../../api/request';
import { createToken, listTokens, revokeToken } from './api';
import '../../i18n';

const id = '123e4567-e89b-12d3-a456-426614174000';
const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);
vi.stubGlobal('document', { cookie: '' });

afterEach(() => fetchMock.mockReset());

const token = {
  id,
  label: 'automation',
  permissions: ['entities.read'],
  expires_at: null,
  revoked_at: null,
  last_used_at: null,
  created_at: '2026-01-01T00:00:00Z',
};

describe('profile API client', () => {
  it('manages personal tokens through the profile feature endpoints', async () => {
    fetchMock.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve([{ ...token }]),
    });
    await listTokens();
    expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/personal-access-tokens');

    fetchMock.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({ ...token, secret: 'cat_pat_only_once' }),
    });
    const created = await createToken({
      label: 'automation',
      permissions: ['entities.read'],
    });
    expect(created.secret).toBe('cat_pat_only_once');

    fetchMock.mockResolvedValueOnce({ ok: true });
    await revokeToken(id);
    expect(fetchMock.mock.calls[2]?.[0]).toBe(
      `/api/personal-access-tokens/${id}`,
    );
  });

  it('uses typed errors and accepts empty token revocation responses', async () => {
    const json = vi.fn();
    fetchMock.mockResolvedValueOnce({ ok: true, status: 204, json });

    await expect(revokeToken(id)).resolves.toBeUndefined();
    expect(json).not.toHaveBeenCalled();

    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 409,
      json: () =>
        Promise.resolve({
          error: { code: 'token_already_revoked', message: 'Token is revoked' },
        }),
    });
    await expect(revokeToken(id)).rejects.toMatchObject({
      name: 'ApiRequestError',
      status: 409,
      code: 'token_already_revoked',
      message: 'Token is revoked',
    } satisfies Partial<ApiRequestError>);
  });

  it('validates token input before submitting it', () => {
    expect(() =>
      createToken({
        label: 'automation',
        permissions: ['entities.read'],
        expires_at: '2020-01-01T00:00:00.000Z',
      }),
    ).toThrow('Token expiry must be in the future');
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
