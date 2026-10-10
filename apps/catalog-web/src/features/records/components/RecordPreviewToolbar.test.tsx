// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import type { RecordPublicationReadiness } from '../schemas';
import { RecordPreviewToolbar } from './RecordPreviewToolbar';

vi.mock('@tanstack/react-router', () => ({
  createLink: (component: unknown) => component,
}));

const storefront = '123e4567-e89b-12d3-a456-426614174000';
const notReady: RecordPublicationReadiness = {
  context_id: storefront,
  context_code: 'storefront',
  ready: false,
  violations: [
    {
      source: 'record_check',
      code: 'has-sku',
      message: 'SKU is required.',
      contexts: [],
      attributes: ['sku'],
    },
  ],
};

const renderToolbar = (
  readiness: RecordPublicationReadiness,
  onPublish = vi.fn(),
) => {
  const wrap = (node: ReactNode) => (
    <QueryClientProvider client={new QueryClient()}>{node}</QueryClientProvider>
  );
  render(
    wrap(
      <RecordPreviewToolbar
        agentPanelOpen={false}
        canDelete={false}
        canPublish
        recordId="123e4567-e89b-12d3-a456-426614174001"
        extensionPanelOpen={false}
        notReadyChannels={readiness.ready ? [] : [readiness]}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
        onOpenAgent={vi.fn()}
        onOpenExtensions={vi.fn()}
        onPublish={onPublish}
        onPublishAll={vi.fn()}
        onUnpublish={vi.fn()}
        publication={{
          context_id: storefront,
          context_code: 'storefront',
          status: 'not_published',
          published_at: null,
          published_by_user_id: null,
        }}
        publicationPending={false}
        readiness={readiness}
        showExtensions={false}
      />,
    ),
  );
  return { onPublish };
};

const openPublicationMenu = () => {
  fireEvent.click(screen.getByRole('button', { name: 'Publication' }));
  return screen.getByRole('menu');
};

afterEach(cleanup);

describe('RecordPreviewToolbar readiness', () => {
  it('disables publishing and lists the failing checks', () => {
    const { onPublish } = renderToolbar(notReady);
    const menu = openPublicationMenu();
    const publish = within(menu).getByRole('menuitem', {
      name: /^Publish SKU/,
    });
    expect(publish.getAttribute('aria-disabled')).toBe('true');
    expect(within(publish).getByText('SKU is required.')).toBeTruthy();
    fireEvent.click(publish);
    expect(onPublish).not.toHaveBeenCalled();
  });

  it('makes the not-ready reasons reachable from the keyboard', async () => {
    renderToolbar(notReady);
    const chip = screen.getByText('Not ready').closest('[tabindex]');
    const user = userEvent.setup();
    while (document.activeElement !== chip && document.activeElement) {
      await user.tab();
      if (document.activeElement === document.body) break;
    }
    expect(document.activeElement).toBe(chip);
    // Closed, the tooltip describes the focused chip through its title.
    expect(chip?.getAttribute('title')).toBe('SKU is required.');
  });

  it('allows publishing a ready record', () => {
    const { onPublish } = renderToolbar({
      ...notReady,
      ready: true,
      violations: [],
    });
    expect(screen.queryByText('Not ready')).toBeNull();
    const menu = openPublicationMenu();
    fireEvent.click(within(menu).getByRole('menuitem', { name: 'Publish' }));
    expect(onPublish).toHaveBeenCalledOnce();
  });
});

describe('record actions menu', () => {
  const renderActions = (
    props: Partial<Parameters<typeof RecordPreviewToolbar>[0]>,
  ) =>
    render(
      <QueryClientProvider client={new QueryClient()}>
        <RecordPreviewToolbar
          agentPanelOpen={false}
          canDelete={false}
          canPublish={false}
          recordId="123e4567-e89b-12d3-a456-426614174001"
          extensionPanelOpen={false}
          onDelete={vi.fn()}
          onDuplicate={vi.fn()}
          onOpenAgent={vi.fn()}
          onOpenExtensions={vi.fn()}
          onPublish={vi.fn()}
          onPublishAll={vi.fn()}
          onUnpublish={vi.fn()}
          publicationPending={false}
          showExtensions={false}
          {...props}
        />
      </QueryClientProvider>,
    );
  const openActions = async () => {
    await userEvent.click(screen.getByRole('button', { name: 'Actions' }));
    return within(screen.getByRole('menu'));
  };

  it('lists record actions with icons and closes after one is chosen', async () => {
    const onDuplicate = vi.fn();
    const onDelete = vi.fn();
    renderActions({ canDelete: true, onDelete, onDuplicate });
    let menu = await openActions();
    expect(
      menu.getAllByRole('menuitem').map((item) => item.textContent),
    ).toEqual([
      'Changes',
      'Copy record ID',
      'Duplicate record',
      'Delete record',
    ]);
    for (const item of menu.getAllByRole('menuitem'))
      expect(item.querySelector('svg')).toBeTruthy();

    await userEvent.click(
      menu.getByRole('menuitem', { name: 'Duplicate record' }),
    );
    expect(onDuplicate).toHaveBeenCalledOnce();
    menu = await openActions();
    await userEvent.click(
      menu.getByRole('menuitem', { name: 'Delete record' }),
    );
    expect(onDelete).toHaveBeenCalledOnce();
  });

  it('copies the record ID', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });
    renderActions({});
    const menu = await openActions();
    await userEvent.click(
      menu.getByRole('menuitem', { name: 'Copy record ID' }),
    );
    expect(writeText).toHaveBeenCalledWith(
      '123e4567-e89b-12d3-a456-426614174001',
    );
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('offers the schema upgrade only for outdated records', async () => {
    renderActions({ schemaOutdated: true });
    const menu = await openActions();
    expect(
      menu.getByRole('menuitem', { name: 'Upgrade blueprint' }),
    ).toBeTruthy();
  });

  it('moves the agent and extensions into the menu when compact', async () => {
    const onOpenAgent = vi.fn();
    const onOpenExtensions = vi.fn();
    renderActions({
      compact: true,
      onOpenAgent,
      onOpenExtensions,
      showExtensions: true,
    });
    expect(
      screen.queryByRole('button', { name: 'Ask about this record' }),
    ).toBeNull();
    expect(
      screen.queryByRole('button', { name: 'Extension contributions' }),
    ).toBeNull();

    let menu = await openActions();
    await userEvent.click(
      menu.getByRole('menuitem', { name: 'Ask about this record' }),
    );
    expect(onOpenAgent).toHaveBeenCalledOnce();
    menu = await openActions();
    await userEvent.click(
      menu.getByRole('menuitem', { name: 'Extension contributions' }),
    );
    expect(onOpenExtensions).toHaveBeenCalledOnce();
  });
});
