// @vitest-environment jsdom
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { ExtensionContribution } from './api';

const { frameMounts } = vi.hoisted(() => ({ frameMounts: vi.fn() }));

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  getExtensionRuntime: vi.fn(),
}));

vi.mock('./ExtensionFrame', async () => {
  const React = await import('react');
  return {
    ExtensionFrame: ({
      contribution,
      onContentHeight,
    }: {
      contribution: { id: string };
      onContentHeight?: (height: number) => void;
    }) => {
      React.useEffect(() => {
        frameMounts();
        onContentHeight?.(48);
        // The frame reports its initial size once after mounting.
        // eslint-disable-next-line react-hooks/exhaustive-deps
      }, []);
      return <div>{contribution.id}</div>;
    },
  };
});

import { getExtensionRuntime } from './api';
import { ExtensionOutlet, ExtensionPopoverOutlet } from './ExtensionOutlet';

const contribution: ExtensionContribution = {
  contribution_key: 'example.extension:row-action',
  display_order: 0,
  capabilities: [],
  configuration: null,
  extension_id: 'example.extension',
  id: 'row-action',
  kind: 'action',
  outlet: 'explorer_row_action',
  release_id: '11111111-1111-4111-8111-111111111111',
  title: 'Example action',
  version: 1,
};

const renderOutlet = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <ExtensionPopoverOutlet
        context={{
          blueprint_id: '22222222-2222-4222-8222-222222222222',
          blueprint_version: 1,
          context_version: 1,
          entity_id: '33333333-3333-4333-8333-333333333333',
        }}
        label="Extension actions"
        outlet="explorer_row_action"
      />
    </QueryClientProvider>,
  );
};

describe('ExtensionOutlet', () => {
  it('keeps one primary and three secondary entity actions before overflow', async () => {
    const actions = Array.from({ length: 5 }, (_, index) => ({
      ...contribution,
      contribution_key: `example.extension:action-${index}`,
      display_order: index,
      id: `action-${index}`,
      outlet: 'entity_action' as const,
      kind: 'embedded' as const,
    }));
    vi.mocked(getExtensionRuntime).mockResolvedValue(actions);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet outlet="entity_action" />
      </QueryClientProvider>,
    );
    await screen.findByText('action-3');
    expect(screen.queryByText('action-4')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'extensions.moreActions' }));
    expect(await screen.findByText('action-4')).toBeTruthy();
  });
});

describe('ExtensionPopoverOutlet', () => {
  it('keeps one mounted frame while measuring and opening the popover', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([contribution]);
    const user = userEvent.setup();

    renderOutlet();

    const button = await screen.findByRole('button', {
      name: 'Extension actions',
    });
    await waitFor(() => expect(frameMounts).toHaveBeenCalledOnce());

    await user.click(button);

    expect(frameMounts).toHaveBeenCalledOnce();
  });
});
