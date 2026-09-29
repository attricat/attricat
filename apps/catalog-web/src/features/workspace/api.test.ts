import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  createInvitation,
  ensureActiveScopeTarget,
  grantMemberRole,
  listMembers,
  listRoles,
  selectedScopeTarget,
} from './api';
import '../../i18n';

const id = '123e4567-e89b-12d3-a456-426614174000';
const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);
vi.stubGlobal('document', { cookie: '' });

afterEach(() => fetchMock.mockReset());

const respond = (body: unknown) =>
  fetchMock.mockResolvedValue({ ok: true, json: () => Promise.resolve(body) });

describe('workspace API client', () => {
  it('loads management catalogues through their feature routes', async () => {
    respond([]);
    await listMembers();
    expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/workspace/members');

    respond([]);
    await listRoles();
    expect(fetchMock.mock.calls[1]?.[0]).toBe('/api/workspace/roles');
  });

  it('rejects malformed API data before it reaches the UI', async () => {
    respond([{ id }]);
    await expect(listMembers()).rejects.toThrow();
  });

  it('uses the session workspace for an entire-workspace scope', () => {
    const workspaceId = '223e4567-e89b-12d3-a456-426614174000';
    expect(selectedScopeTarget('workspace', id, workspaceId)).toBe(workspaceId);
    expect(selectedScopeTarget('entity', id, workspaceId)).toBe(id);
  });

  it('rejects stale and cross-scope grant targets before posting a mutation', async () => {
    expect(() =>
      ensureActiveScopeTarget(
        { role_id: id, scope_type: 'entity', scope_target_id: id },
        '223e4567-e89b-12d3-a456-426614174000',
        [],
      ),
    ).toThrow('Choose a target');
    expect(() =>
      ensureActiveScopeTarget(
        { role_id: id, scope_type: 'workspace', scope_target_id: id },
        '223e4567-e89b-12d3-a456-426614174000',
        [],
      ),
    ).toThrow('active workspace');
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('validates mutation request shapes before serializing them', async () => {
    expect(() =>
      grantMemberRole('not-a-uuid', {
        role_id: id,
        scope_type: 'workspace',
        scope_target_id: id,
      }),
    ).toThrow();
    expect(() =>
      createInvitation({
        email: 'invitee@example.com',
        role_id: id,
        scope_type: 'workspace',
        scope_target_id: id,
        expires_at: 'not-a-date',
      }),
    ).toThrow();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
