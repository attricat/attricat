// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import type { EntityPublicationReadiness } from '../schemas';
import { EntityPreviewToolbar } from './EntityPreviewToolbar';

vi.mock('@tanstack/react-router', () => ({
  createLink: (component: unknown) => component,
}));

const storefront = '123e4567-e89b-12d3-a456-426614174000';
const notReady: EntityPublicationReadiness = {
  context_id: storefront,
  context_code: 'storefront',
  ready: false,
  violations: [
    {
      source: 'entity_check',
      code: 'has-sku',
      message: 'SKU is required.',
      contexts: [],
      attributes: ['sku'],
    },
  ],
};

const renderToolbar = (
  readiness: EntityPublicationReadiness,
  onPublish = vi.fn(),
) => {
  const wrap = (node: ReactNode) => (
    <QueryClientProvider client={new QueryClient()}>{node}</QueryClientProvider>
  );
  render(
    wrap(
      <EntityPreviewToolbar
        agentPanelOpen={false}
        canDelete={false}
        canPublish
        duplicatePending={false}
        entityId="123e4567-e89b-12d3-a456-426614174001"
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

describe('EntityPreviewToolbar readiness', () => {
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

  it('makes the not-ready reasons reachable from the keyboard', () => {
    renderToolbar(notReady);
    const chip = screen.getByText('Not ready').closest('[tabindex]');
    expect(chip?.getAttribute('tabindex')).toBe('0');
    expect(chip?.getAttribute('title')).toBe('SKU is required.');
  });

  it('allows publishing a ready entity', () => {
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
