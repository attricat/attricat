import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import type { ExtensionContribution } from '../api';
import { handleBrokerRequest } from '../extensionBroker';
import {
  brokerOutcomes,
  brokerRequest,
  recordDrop,
  useExtensionDiagnosticsStore,
} from './extensionDiagnosticsStore';

vi.mock('../extensionBroker', () => ({ handleBrokerRequest: vi.fn() }));

const dependencies = {
  contribution: {
    id: 'panel',
    extension_name: 'Acme',
  } as ExtensionContribution,
  initialContext: {},
  currentContext: () => ({}),
  navigateToRecord: vi.fn(),
  queryClient: new QueryClient(),
  closeActionDialog: vi.fn(),
  onRunStarted: vi.fn(),
  downloadArtifact: vi.fn(),
};

describe('extension diagnostics', () => {
  it('records broker requests without their contents, and why one was denied', async () => {
    useExtensionDiagnosticsStore.setState({ requests: [] });
    vi.mocked(handleBrokerRequest).mockResolvedValueOnce({ secret: 'answer' });
    vi.mocked(handleBrokerRequest).mockRejectedValueOnce(
      new Error('Request denied'),
    );
    await brokerRequest(
      { method: 'command', payload: { secret: 'input' } },
      dependencies,
    );
    await expect(
      brokerRequest({ method: 'navigate', payload: {} }, dependencies),
    ).rejects.toThrow('Request denied');
    const [denied, allowed] = useExtensionDiagnosticsStore.getState().requests;
    expect(allowed).toMatchObject({
      method: 'command',
      outcome: brokerOutcomes.allowed,
    });
    expect(denied).toMatchObject({
      method: 'navigate',
      outcome: brokerOutcomes.denied,
      reason: 'Request denied',
    });
    expect(JSON.stringify([allowed, denied])).not.toContain('secret');
  });

  it('keeps a bounded history of what was dropped', () => {
    useExtensionDiagnosticsStore.setState({ drops: [] });
    for (let index = 0; index < 60; index += 1)
      recordDrop('Acme', `drop ${index}`);
    const { drops } = useExtensionDiagnosticsStore.getState();
    expect(drops).toHaveLength(50);
    expect(drops[0].message).toBe('drop 59');
  });
});
