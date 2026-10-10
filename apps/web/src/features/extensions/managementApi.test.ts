import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  installExtension,
  installedExtensions,
  updateWorkspaceExtensionLayout,
  workspaceExtensionLayout,
} from './managementApi';

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);
vi.stubGlobal('document', { cookie: '' });
afterEach(() => fetchMock.mockReset());

const installation = {
  id: '123e4567-e89b-12d3-a456-426614174000',
  extension_id: 'acme.test',
  installed_release_id: '123e4567-e89b-12d3-a456-426614174001',
  state: 'disabled',
  configuration: {},
  configuration_version: null,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
  version: '1.0.0',
  manifest: {},
  manifest_sha256: 'a'.repeat(64),
  source: 'github:acme/test@v1.0.0',
};

describe('extension management API', () => {
  it('installs a registry-resolved release without accepting a download URL', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(installation),
    });
    await expect(
      installExtension({ owner: 'acme', repository: 'test', release_id: 42 }),
    ).resolves.toEqual(installation);
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/extensions',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          owner: 'acme',
          repository: 'test',
          release_id: 42,
        }),
      }),
    );
  });

  it('loads and saves the versioned host-owned workspace layout', async () => {
    const layout = {
      version: 1 as const,
      outlets: {
        record_preview_panel: {
          order: ['acme.test:panel'],
          hidden: ['acme.legacy:panel'],
        },
      },
    };
    fetchMock.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(layout),
    });
    await expect(workspaceExtensionLayout()).resolves.toEqual(layout);
    fetchMock.mockResolvedValueOnce({ ok: true, status: 204 });
    await expect(
      updateWorkspaceExtensionLayout(layout),
    ).resolves.toBeUndefined();
    expect(fetchMock).toHaveBeenLastCalledWith(
      '/api/workspace/extension-layout',
      expect.objectContaining({ method: 'PUT', body: JSON.stringify(layout) }),
    );
  });

  it('rejects malformed installed extension responses', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([{ ...installation, state: 'unknown' }]),
    });
    await expect(installedExtensions()).rejects.toThrow();
  });
});
