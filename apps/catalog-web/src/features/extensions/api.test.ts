import { afterEach, describe, expect, it, vi } from 'vitest';
import { getExtensionArtifact, getExtensionRuntime } from './api';

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);
vi.stubGlobal('document', { cookie: '' });
afterEach(() => fetchMock.mockReset());

const contribution = {
  extension_id: 'acme.test',
  release_id: '123e4567-e89b-12d3-a456-426614174000',
  configuration: {},
  capabilities: ['client.navigation'],
  id: 'panel',
  version: 1,
  kind: 'element',
  outlet: 'entity_preview_panel',
  title: null,
  element: 'acme-panel',
};

describe('extension runtime API', () => {
  it('validates runtime descriptors before exposing them to a frame', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([contribution]),
    });
    await expect(getExtensionRuntime()).resolves.toEqual([contribution]);
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([{ ...contribution, element: undefined }]),
    });
    await expect(getExtensionRuntime()).rejects.toThrow();
  });

  it('addresses artifacts by declared contribution instead of storage key', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      text: () => Promise.resolve('export {}'),
    });
    await getExtensionArtifact('acme.test', 'panel');
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/extensions/acme.test/panel/artifact',
    );
  });
});
