// @vitest-environment jsdom
import { QueryClient } from '@tanstack/react-query';
import { ThreadMessagePort } from '@quilted/threads';
import {
  MUTATION_TYPE_INSERT_CHILD,
  NODE_TYPE_ELEMENT,
  NODE_TYPE_TEXT,
  ROOT_ID,
  type RemoteConnection,
  type RemoteElementSerialization,
} from '@remote-dom/core';
import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ExtensionContribution } from '../api';
import { handleBrokerRequest } from '../extensionBroker';
import { nativeAdapters } from './adapterMap';
import {
  nativeMountStates,
  nativeUiBudgets,
  nativeUiErrorCodes,
} from './constants';
import { uiKitElements } from './generated/uiKit';
import {
  NativeExtensionHost,
  type NativeIsolation,
  type NativeMountInit,
} from './nativeRuntime';
import { NativeTree } from './NativeTree';
import { nativeSandboxDocument } from './sandboxDocument';
import contract from '../../../../../../contracts/ui-kit-v1.schema.json';

vi.mock('../api', () => ({
  getExtensionArtifact: vi.fn(async () => 'export {}'),
}));
vi.mock('../extensionBroker', () => ({
  handleBrokerRequest: vi.fn(async () => ({ ok: true })),
}));

const contribution = (
  extensionId: string,
  id = 'panel',
): ExtensionContribution => ({
  contribution_key: `${extensionId}:${id}`,
  display_order: 0,
  navigation_group: null,
  capabilities: ['client.commands'],
  configuration: null,
  extension_id: extensionId,
  extension_name: extensionId,
  id,
  kind: 'panel',
  outlet: 'blueprint_panel',
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
  title: null,
  version: 1,
  invoke: null,
  when: null,
  confirm: null,
  icon: null,
  translations: {},
  cell: null,
  editor: null,
  rendering: 'native',
  ui_kit: '1.0.0',
  artifact_revision: null,
});

const init: NativeMountInit = {
  context: {},
  configuration: null,
  capabilities: [],
  locale: 'en',
  theme: { color_mode: 'light' },
  ui_kit: '1.0.0',
};

const dependencies = (item: ExtensionContribution) => ({
  contribution: item,
  initialContext: {},
  currentContext: () => ({}),
  navigateToRecord: vi.fn(),
  queryClient: new QueryClient(),
  closeActionDialog: vi.fn(),
  onRunStarted: vi.fn(),
  downloadArtifact: vi.fn(),
});

const element = (
  id: string,
  tag: string,
  properties: Record<string, unknown> = {},
  children: RemoteElementSerialization['children'] = [],
): RemoteElementSerialization => ({
  id,
  type: NODE_TYPE_ELEMENT,
  element: tag,
  properties,
  attributes: {},
  eventListeners: {},
  children,
});

type FakeWorker = {
  /** Registers these contribution IDs; all of them by default. */
  registered?: string[];
  /** Draws into each mount's connection. */
  draw?: (connection: RemoteConnection) => void;
  /** A hung Worker never answers a heartbeat. */
  hung?: boolean;
};

/** Runs a fake SDK on the extension's port, in process. */
const isolation = (
  worker: FakeWorker,
  onWorker: (host: {
    request: (...args: unknown[]) => Promise<unknown>;
  }) => void = () => {},
): NativeIsolation & { stop: ReturnType<typeof vi.fn> } => ({
  stop: vi.fn(),
  start: async (_key, _artifact, port) => {
    const thread = new ThreadMessagePort<
      {
        ready: (ids: string[]) => Promise<void>;
        request: (...args: unknown[]) => Promise<unknown>;
      },
      Record<string, (...args: never[]) => Promise<unknown>>
    >(port, {
      exports: {
        mount: async (
          _mountId: string,
          _id: string,
          connection: RemoteConnection,
        ) => worker.draw?.(connection),
        update: async () => {},
        unmount: async () => {},
        ping: () => (worker.hung ? new Promise(() => {}) : Promise.resolve()),
      },
    });
    port.start();
    onWorker(thread.imports);
    await thread.imports.ready(worker.registered ?? ['panel']);
  },
});

afterEach(() => {
  vi.useRealTimers();
  vi.mocked(handleBrokerRequest).mockClear();
});

describe('native extension host', () => {
  it('has a host adapter for every UI kit element', () => {
    for (const name of Object.keys(contract.elements))
      expect(nativeAdapters, name).toHaveProperty(name);
    expect(Object.keys(uiKitElements).sort()).toEqual(
      Object.keys(contract.elements).sort(),
    );
  });

  it('starts every Worker in an opaque sandbox without network access', () => {
    expect(nativeSandboxDocument).toContain("connect-src 'none'");
    expect(nativeSandboxDocument).toContain('worker-src blob:');
    expect(nativeSandboxDocument).toContain("default-src 'none'");
  });

  it('draws only UI kit elements and their declared props', async () => {
    const item = contribution('acme.one');
    const host = new NativeExtensionHost(
      [item],
      isolation({
        draw: (connection) =>
          connection.mutate([
            [
              MUTATION_TYPE_INSERT_CHILD,
              ROOT_ID,
              element(
                '1',
                'attricat-stack',
                { gap: 'sm', style: 'position:fixed' },
                [
                  element('2', 'attricat-text', { tone: 'muted' }, [
                    { id: '3', type: NODE_TYPE_TEXT, data: 'Kit text' },
                  ]),
                  element('4', 'div', {}, [
                    { id: '5', type: NODE_TYPE_TEXT, data: 'Raw HTML' },
                  ]),
                  element(
                    '6',
                    'attricat-link',
                    { href: 'javascript:alert(1)' },
                    [{ id: '7', type: NODE_TYPE_TEXT, data: 'Bad link' }],
                  ),
                  element('8', 'attricat-text-field', {}),
                ],
              ),
              0,
            ],
          ]),
      }),
    );
    const { mountId, receiver } = host.mount(item, init, dependencies(item));
    await vi.waitFor(() =>
      expect(host.snapshot(mountId).state).toBe(nativeMountStates.ready),
    );
    render(<NativeTree receiver={receiver} />);
    expect(screen.getByText('Kit text')).toBeTruthy();
    // Unknown elements, unsafe links and inputs without a label never draw.
    expect(screen.queryByText('Raw HTML')).toBeNull();
    expect(screen.queryByText('Bad link')).toBeNull();
    expect(screen.queryByRole('textbox')).toBeNull();
    const stack = receiver.node('1');
    expect(stack?.kind === 'element' && stack.props).toEqual({ gap: 'sm' });
    host.terminate();
  });

  it("attributes requests to the port's own mounts, never another extension's", async () => {
    const one = contribution('acme.one');
    const two = contribution('acme.two');
    let workerOne:
      { request: (...args: unknown[]) => Promise<unknown> } | undefined;
    const hostOne = new NativeExtensionHost(
      [one],
      isolation({}, (host) => (workerOne = host)),
    );
    const hostTwo = new NativeExtensionHost([two], isolation({}));
    hostOne.mount(one, init, dependencies(one));
    hostTwo.mount(two, init, dependencies(two));
    hostTwo.mount(two, init, dependencies(two));
    await vi.waitFor(() =>
      expect(hostTwo.snapshot('2').state).toBe(nativeMountStates.ready),
    );
    await expect(
      workerOne?.request('1', 'command', { command_id: 'save' }),
    ).resolves.toEqual({ ok: true });
    expect(
      vi.mocked(handleBrokerRequest).mock.calls[0]?.[1].contribution
        .extension_id,
    ).toBe('acme.one');
    // Mount 2 exists only on the other extension's port.
    await expect(
      workerOne?.request('2', 'command', { command_id: 'save' }),
    ).rejects.toThrow();
    expect(handleBrokerRequest).toHaveBeenCalledTimes(1);
    hostOne.terminate();
    hostTwo.terminate();
  });

  it('rejects registrations of undeclared contributions', async () => {
    const declared = contribution('acme.one', 'declared');
    const host = new NativeExtensionHost(
      [declared],
      isolation({ registered: ['undeclared'] }),
    );
    const { mountId } = host.mount(declared, init, dependencies(declared));
    await vi.waitFor(() =>
      expect(host.snapshot(mountId)).toEqual({
        state: nativeMountStates.failed,
        error: nativeUiErrorCodes.notRegistered,
      }),
    );
    host.terminate();
  });

  it('terminates an over-budget Worker without touching another extension', async () => {
    const one = contribution('acme.one');
    const two = contribution('acme.two');
    const tooMany = Array.from(
      { length: nativeUiBudgets.maximumTreeNodes + 1 },
      (_, index) => element(String(index + 1), 'attricat-divider'),
    );
    const sandbox = isolation({
      draw: (connection) =>
        connection.mutate([
          [
            MUTATION_TYPE_INSERT_CHILD,
            ROOT_ID,
            element('0', 'attricat-stack', {}, tooMany),
            0,
          ],
        ]),
    });
    const greedy = new NativeExtensionHost([one], sandbox);
    const healthy = new NativeExtensionHost([two], isolation({}));
    const greedyMount = greedy.mount(one, init, dependencies(one));
    const healthyMount = healthy.mount(two, init, dependencies(two));
    await vi.waitFor(() => {
      expect(greedy.snapshot(greedyMount.mountId).error).toBe(
        nativeUiErrorCodes.treeTooLarge,
      );
      expect(healthy.snapshot(healthyMount.mountId).state).toBe(
        nativeMountStates.ready,
      );
    });
    expect(sandbox.stop).toHaveBeenCalled();
    expect(healthy.snapshot(healthyMount.mountId).state).toBe(
      nativeMountStates.ready,
    );
    healthy.terminate();
  });

  it('terminates a Worker that stops answering heartbeats', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const item = contribution('acme.one');
    const sandbox = isolation({ hung: true });
    const host = new NativeExtensionHost([item], sandbox);
    const { mountId } = host.mount(item, init, dependencies(item));
    await vi.advanceTimersByTimeAsync(50);
    await vi.waitFor(() =>
      expect(host.snapshot(mountId).state).toBe(nativeMountStates.ready),
    );
    await vi.advanceTimersByTimeAsync(
      nativeUiBudgets.heartbeatInterval + nativeUiBudgets.heartbeatTimeout + 10,
    );
    expect(host.snapshot(mountId).error).toBe(nativeUiErrorCodes.unresponsive);
    expect(sandbox.stop).toHaveBeenCalled();
  });
});
