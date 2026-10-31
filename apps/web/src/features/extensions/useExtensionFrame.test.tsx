// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ExtensionContribution } from './api';
import { extensionMessageTypes } from './constants';
import { useExtensionFrame } from './useExtensionFrame';

vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('./api', () => ({
  getExtensionArtifact: vi.fn(async () => 'export const mount = () => {};'),
}));

const contribution: ExtensionContribution = {
  contribution_key: 'example.extension:panel',
  display_order: 0,
  navigation_group: null,
  capabilities: [],
  configuration: { mode: 'compact' },
  extension_id: 'example.extension',
  extension_name: 'Example Extension',
  id: 'panel',
  kind: 'panel',
  outlet: 'blueprint_panel',
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
  title: 'Example panel',
  version: 1,
  invoke: null,
  when: null,
  confirm: null,
  icon: null,
  translations: {},
  cell: null,
};

const Frame = ({ contribution }: { contribution: ExtensionContribution }) => {
  const { frameKey, handleFrameLoad, iframeRef } = useExtensionFrame({
    contribution,
    context: {},
  });
  return (
    <iframe
      key={frameKey}
      onLoad={handleFrameLoad}
      ref={iframeRef}
      title="frame"
    />
  );
};

/** Loads the current frame and returns its `postMessage` spy. */
const loadFrame = () => {
  const frame = screen.getByTitle('frame') as HTMLIFrameElement;
  const postMessage = vi.spyOn(frame.contentWindow!, 'postMessage');
  fireEvent.load(frame);
  return { frame, postMessage };
};

const inits = (postMessage: ReturnType<typeof vi.spyOn>) =>
  postMessage.mock.calls.filter(
    ([message]) =>
      (message as { type?: string }).type === extensionMessageTypes.init,
  );

describe('useExtensionFrame', () => {
  it('loads a new frame instead of initialising the same one twice', async () => {
    const client = new QueryClient();
    const tree = (next: ExtensionContribution) => (
      <QueryClientProvider client={client}>
        <Frame contribution={next} />
      </QueryClientProvider>
    );
    const { rerender } = render(tree(contribution));
    const first = loadFrame();
    await waitFor(() => expect(inits(first.postMessage)).toHaveLength(1));

    // A refetch with an equal contribution keeps the running frame.
    rerender(tree({ ...contribution }));
    expect(screen.getByTitle('frame')).toBe(first.frame);

    rerender(tree({ ...contribution, configuration: { mode: 'wide' } }));
    expect(screen.getByTitle('frame')).not.toBe(first.frame);
    const second = loadFrame();
    await waitFor(() => expect(inits(second.postMessage)).toHaveLength(1));
    expect(inits(second.postMessage)[0][0]).toMatchObject({
      configuration: { mode: 'wide' },
    });
    expect(inits(first.postMessage)).toHaveLength(1);
  });
});
