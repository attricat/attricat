import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import type { ExtensionContribution } from './api';
import {
  handleBrokerRequest,
  postBrokerResponse,
  validateStorageRequest,
} from './extensionBroker';
import { maximumExtensionResponseBytes } from './constants';

vi.mock('../../api/request', () => ({ requestText: vi.fn() }));

const contribution: ExtensionContribution = {
  contribution_key: 'example.extension:panel',
  display_order: 0,
  navigation_group: null,
  capabilities: [],
  configuration: null,
  extension_id: 'example.extension',
  extension_name: 'Example Extension',
  id: 'panel',
  kind: 'panel',
  outlet: 'blueprint_panel',
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
  title: null,
  version: 1,
};

const dependencies = (capabilities: string[]) => ({
  contribution: { ...contribution, capabilities },
  initialContext: {
    blueprint_id: '22222222-2222-4222-8222-222222222222',
    blueprint_version: 1,
  },
  currentContext: () => ({}),
  navigateToEntity: vi.fn(),
  queryClient: new QueryClient(),
});

describe('extension broker', () => {
  it('denies methods without the matching capability', async () => {
    const deps = dependencies([]);
    await expect(
      handleBrokerRequest(
        {
          method: 'navigate',
          payload: { entity_id: '33333333-3333-4333-8333-333333333333' },
        },
        deps,
      ),
    ).rejects.toThrow('Request denied');
    expect(deps.navigateToEntity).not.toHaveBeenCalled();
  });

  it('denies commands from panel contributions', async () => {
    await expect(
      handleBrokerRequest(
        { method: 'command', payload: { command_id: 'run' } },
        dependencies(['client.commands']),
      ),
    ).rejects.toThrow('Request denied');
  });

  it('denies blueprint reads outside the outlet revision', async () => {
    await expect(
      handleBrokerRequest(
        {
          method: 'catalog.read',
          payload: {
            path: '/api/blueprints/22222222-2222-4222-8222-222222222222/versions/2',
          },
        },
        dependencies(['catalog.read']),
      ),
    ).rejects.toThrow('Blueprint revision is outside this outlet context');
  });

  it('rejects storage keys with control characters', () => {
    expect(() =>
      validateStorageRequest({ operation: 'get', key: 'bad\u0007key' }),
    ).toThrow('Invalid storage key');
  });

  it('denies oversized successful responses', () => {
    const port = { postMessage: vi.fn() } as unknown as MessagePort;
    postBrokerResponse(port, '1', {
      ok: true,
      data: 'x'.repeat(maximumExtensionResponseBytes),
    });
    expect(port.postMessage).toHaveBeenCalledWith({
      type: 'catalog:response.v1',
      id: '1',
      ok: false,
      error: 'Request denied',
    });
  });
});
