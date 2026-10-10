import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import * as runs from '../extension-runs/api';
import type { ExtensionContribution } from './api';
import {
  handleBrokerRequest,
  postBrokerResponse,
  validateStorageRequest,
} from './extensionBroker';
import { maximumExtensionResponseBytes } from './constants';

vi.mock('../../api/request', () => ({ requestText: vi.fn() }));
vi.mock('../extension-runs/api', () => ({
  cancelExtensionRun: vi.fn(),
  extensionRunArtifactUrl: (
    runId: string,
    artifactId: string,
    scope?: string,
  ) =>
    `/api/extension-runs/${runId}/artifacts/${artifactId}/download${scope === 'own' ? '?scope=own' : ''}`,
  getExtensionRun: vi.fn(),
  listExtensionRuns: vi.fn(),
  startExtensionRun: vi.fn(),
}));

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
  navigateToRecord: vi.fn(),
  queryClient: new QueryClient(),
  openActionDialog: vi.fn(),
  closeActionDialog: vi.fn(),
  onRunStarted: vi.fn(),
  downloadArtifact: vi.fn(),
});

const selection = {
  context_version: 2,
  selection_source: 'explorer_selection',
  blueprint_id: '22222222-2222-4222-8222-222222222222',
  blueprint_version: 3,
  context_id: null,
  record_ids: [
    '33333333-3333-4333-8333-333333333333',
    '44444444-4444-4444-8444-444444444444',
  ],
};

const selectionDependencies = (
  capabilities: string[],
  overrides: Partial<ExtensionContribution> = {},
) => ({
  ...dependencies(capabilities),
  contribution: {
    ...contribution,
    capabilities,
    id: 'bulk',
    kind: 'action' as const,
    outlet: 'explorer_bulk_action' as const,
    version: 2,
    ...overrides,
  },
  currentContext: () => selection,
});

describe('extension broker', () => {
  it('denies methods without the matching capability', async () => {
    const deps = dependencies([]);
    await expect(
      handleBrokerRequest(
        {
          method: 'navigate',
          payload: { record_id: '33333333-3333-4333-8333-333333333333' },
        },
        deps,
      ),
    ).rejects.toThrow('Request denied');
    expect(deps.navigateToRecord).not.toHaveBeenCalled();
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

  it('opens the action dialog with the captured selection only for v2 actions', async () => {
    const deps = selectionDependencies(['client.action_dialog']);
    await handleBrokerRequest({ method: 'dialog.open', payload: {} }, deps);
    expect(deps.openActionDialog).toHaveBeenCalledWith({
      extensionId: 'example.extension',
      context: selection,
    });
    const legacy = selectionDependencies(['client.action_dialog'], {
      version: 1,
    });
    await expect(
      handleBrokerRequest({ method: 'dialog.open', payload: {} }, legacy),
    ).rejects.toThrow('Request denied');
    expect(legacy.openActionDialog).not.toHaveBeenCalled();
  });

  it('starts runs for the host-captured selection, never a frame-supplied one', async () => {
    vi.mocked(runs.startExtensionRun).mockResolvedValue({
      run_id: '55555555-5555-4555-8555-555555555555',
    });
    const deps = selectionDependencies(['client.operations.start']);
    const result = await handleBrokerRequest(
      {
        method: 'operations.start',
        payload: {
          operation_id: 'generate',
          input: { template: 'summary' },
          idempotency_key: 'generate-1',
        },
      },
      deps,
    );
    expect(result).toEqual({ run_id: '55555555-5555-4555-8555-555555555555' });
    expect(runs.startExtensionRun).toHaveBeenCalledWith(
      'example.extension',
      'bulk',
      expect.objectContaining({
        release_id: contribution.release_id,
        selection: {
          blueprint_id: selection.blueprint_id,
          blueprint_version: 3,
          context_id: null,
          record_ids: selection.record_ids,
        },
      }),
    );
    expect(deps.onRunStarted).toHaveBeenCalled();
    await expect(
      handleBrokerRequest(
        {
          method: 'operations.start',
          payload: {
            operation_id: 'generate',
            input: {},
            idempotency_key: 'generate-2',
            record_ids: ['66666666-6666-4666-8666-666666666666'],
          },
        },
        deps,
      ),
    ).rejects.toThrow();
  });

  it('denies operations without the matching capability or outside the extension', async () => {
    await expect(
      handleBrokerRequest(
        { method: 'operations.list', payload: {} },
        selectionDependencies(['client.operations.start']),
      ),
    ).rejects.toThrow('Request denied');
    vi.mocked(runs.getExtensionRun).mockResolvedValue({
      extension_id: 'other.extension',
    } as Awaited<ReturnType<typeof runs.getExtensionRun>>);
    const deps = selectionDependencies(['client.operations.cancel']);
    await expect(
      handleBrokerRequest(
        {
          method: 'operations.cancel',
          payload: { run_id: '55555555-5555-4555-8555-555555555555' },
        },
        deps,
      ),
    ).rejects.toThrow('Request denied');
    expect(runs.cancelExtensionRun).not.toHaveBeenCalled();
  });

  it('reads, cancels and downloads only runs the signed-in user started', async () => {
    const runId = '55555555-5555-4555-8555-555555555555';
    const artifactId = '77777777-7777-4777-8777-777777777777';
    const ownRun = {
      extension_id: 'example.extension',
      initiated_by_me: true,
      artifacts: [{ id: artifactId }],
    } as Awaited<ReturnType<typeof runs.getExtensionRun>>;
    vi.mocked(runs.getExtensionRun).mockResolvedValue(ownRun);
    const deps = selectionDependencies([
      'client.operations.read',
      'client.operations.cancel',
    ]);
    await expect(
      handleBrokerRequest(
        { method: 'operations.get', payload: { run_id: runId } },
        deps,
      ),
    ).resolves.toBe(ownRun);
    expect(runs.getExtensionRun).toHaveBeenLastCalledWith(runId, 'own');
    await handleBrokerRequest(
      { method: 'operations.cancel', payload: { run_id: runId } },
      deps,
    );
    expect(runs.cancelExtensionRun).toHaveBeenLastCalledWith(runId, 'own');
    await handleBrokerRequest(
      {
        method: 'operations.download',
        payload: { run_id: runId, artifact_id: artifactId },
      },
      deps,
    );
    expect(deps.downloadArtifact).toHaveBeenCalledWith(
      `/api/extension-runs/${runId}/artifacts/${artifactId}/download?scope=own`,
    );

    // An operator's frame must not reach another user's run of the same
    // extension, even if the server returned it.
    vi.mocked(runs.cancelExtensionRun).mockClear();
    vi.mocked(runs.getExtensionRun).mockResolvedValue({
      ...ownRun,
      initiated_by_me: false,
    });
    for (const request of [
      { method: 'operations.get', payload: { run_id: runId } },
      { method: 'operations.cancel', payload: { run_id: runId } },
      {
        method: 'operations.download',
        payload: { run_id: runId, artifact_id: artifactId },
      },
    ])
      await expect(handleBrokerRequest(request, deps)).rejects.toThrow(
        'Request denied',
      );
    expect(runs.cancelExtensionRun).not.toHaveBeenCalled();
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
