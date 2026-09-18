import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  extensionCommand,
  extensionStorage,
  getExtensionArtifact,
  getExtensionRuntime,
} from './api';
import { maximumExtensionResponseBytes } from './constants';

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);
vi.stubGlobal('document', { cookie: '' });
afterEach(() => fetchMock.mockReset());

const contribution = {
  contribution_key: 'acme.test:panel',
  display_order: 0,
  navigation_group: null,
  extension_id: 'acme.test',
  extension_name: 'Acme Test',
  release_id: '123e4567-e89b-12d3-a456-426614174000',
  configuration: {},
  capabilities: ['client.navigation'],
  id: 'panel',
  version: 1,
  kind: 'embedded',
  outlet: 'entity_preview_panel',
  route: null,
  title: null,
};

describe('extension runtime API', () => {
  it('validates runtime descriptors before exposing them to a frame', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([contribution]),
    });
    await expect(getExtensionRuntime()).resolves.toEqual([contribution]);
    const action = {
      ...contribution,
      capabilities: ['client.explorer_row_action'],
      kind: 'action',
      outlet: 'explorer_row_action',
    };
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([action]),
    });
    await expect(getExtensionRuntime()).resolves.toEqual([action]);
    const tableCell = {
      ...contribution,
      capabilities: ['client.explorer_table_cell'],
      id: 'example.currency',
      outlet: 'explorer_table_cell',
    };
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([tableCell]),
    });
    await expect(getExtensionRuntime()).resolves.toEqual([tableCell]);
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([{ ...contribution, id: undefined }]),
    });
    await expect(getExtensionRuntime()).rejects.toThrow();
  });

  it('brokers bounded commands through a declared contribution', async () => {
    const result = { arbitrary: [null, 'value', { nested: true }] };
    fetchMock.mockResolvedValue({
      ok: true,
      text: () => Promise.resolve(JSON.stringify(result)),
    });
    await expect(
      extensionCommand('acme.test', 'panel', {
        release_id: contribution.release_id,
        command_id: 'refresh',
        payload: {},
      }),
    ).resolves.toEqual(result);
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/extensions/acme.test/panel/command',
      expect.objectContaining({ method: 'POST' }),
    );
  });

  it('returns valid arbitrary JSON from storage responses', async () => {
    const result = [null, false, { extensible: ['value'] }];
    fetchMock.mockResolvedValue({
      ok: true,
      text: () => Promise.resolve(JSON.stringify(result)),
    });
    await expect(
      extensionStorage('acme.test', 'panel', contribution.release_id, {
        operation: 'get',
        key: 'theme',
      }),
    ).resolves.toEqual(result);
  });

  it('identifies malformed JSON returned by commands and storage', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      text: () => Promise.resolve('{invalid'),
    });
    await expect(
      extensionCommand('acme.test', 'panel', {
        release_id: contribution.release_id,
        command_id: 'refresh',
        payload: {},
      }),
    ).rejects.toThrow('Extension command response contains malformed JSON');
    await expect(
      extensionStorage('acme.test', 'panel', contribution.release_id, {
        operation: 'get',
        key: 'theme',
      }),
    ).rejects.toThrow('Extension storage response contains malformed JSON');
  });

  it('rejects storage responses larger than the byte limit', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      text: () => Promise.resolve('😀'.repeat(maximumExtensionResponseBytes)),
    });
    await expect(
      extensionStorage('acme.test', 'panel', contribution.release_id, {
        operation: 'get',
        key: 'theme',
      }),
    ).rejects.toThrow('Extension storage response is too large');
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
