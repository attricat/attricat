// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import { listContexts } from '../contexts/api';
import { EntityPreviewPage } from './EntityPreviewPage';

const { drawerRender, outletRender, popoverOutletRender } = vi.hoisted(() => ({
  drawerRender: vi.fn(),
  outletRender: vi.fn(),
  popoverOutletRender: vi.fn(),
}));

vi.mock('@tanstack/react-router', () => ({
  createLink: <T,>(component: T) => component,
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
}));

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  getBlueprintRevision: vi.fn(),
  getCurrentBlueprint: vi.fn(),
  getEntityPublications: vi.fn(),
  getResolvedEntityPreview: vi.fn(),
  publishEntity: vi.fn(),
  publishEntityAllChannels: vi.fn(),
  unpublishEntity: vi.fn(),
}));

vi.mock('../extensions/ExtensionOutlet', () => ({
  ExtensionOutlet: (props: unknown) => {
    outletRender(props);
    return null;
  },
  ExtensionPopoverOutlet: (props: unknown) => {
    popoverOutletRender(props);
    return null;
  },
}));

vi.mock('./components/EntityExtensionDrawer', () => ({
  EntityExtensionDrawer: (props: unknown) => {
    drawerRender(props);
    return null;
  },
}));

vi.mock('../views/components/EntityView', () => ({
  EntityView: ({
    renderAttributeDecoration,
  }: {
    renderAttributeDecoration: (attribute: {
      id: string;
      code: string;
    }) => React.ReactNode;
  }) => renderAttributeDecoration({ id: 'attribute-id', code: 'title' }),
}));

vi.mock('../contexts/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../contexts/api')>()),
  listContexts: vi.fn(),
}));

const renderPage = (relationshipPickerToken?: string) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <EntityPreviewPage
          entityId="00000000-0000-4000-8000-000000000001"
          relationshipPickerToken={relationshipPickerToken}
        />
      </ToastProvider>
    </QueryClientProvider>,
  );
};

describe('EntityPreviewPage', () => {
  beforeEach(async () => {
    const api = await import('./api');
    vi.mocked(api.getEntityPublications).mockResolvedValue([]);
  });
  it('returns a picker selection to its opener and closes the preview', () => {
    const postMessage = vi.fn();
    const close = vi.spyOn(window, 'close').mockImplementation(() => undefined);
    Object.defineProperty(window, 'opener', {
      configurable: true,
      value: { postMessage },
    });
    vi.mocked(listContexts).mockResolvedValue([]);

    renderPage('picker-token');
    screen
      .getByRole('button', { name: 'Select this entity and close' })
      .click();

    expect(postMessage).toHaveBeenCalledWith(
      {
        type: 'attricat.relationship-picker.select',
        token: 'picker-token',
        entityId: '00000000-0000-4000-8000-000000000001',
      },
      window.location.origin,
    );
    expect(close).toHaveBeenCalled();

    close.mockRestore();
    Object.defineProperty(window, 'opener', {
      configurable: true,
      value: null,
    });
  });

  it('scopes every blueprint-owned extension surface to the pinned revision', async () => {
    const api = await import('./api');
    const blueprintId = '22222222-2222-4222-8222-222222222222';
    vi.mocked(listContexts).mockResolvedValue([
      {
        id: '33333333-3333-4333-8333-333333333333',
        code: 'default',
        data: {},
        parent_id: null,
      },
    ]);
    vi.mocked(api.getResolvedEntityPreview).mockResolvedValue({
      entity: {
        id: '00000000-0000-4000-8000-000000000001',
        blueprint_id: blueprintId,
        blueprint_version: 7,
      },
      values: {},
    } as never);
    const blueprint = {
      blueprint: {
        id: blueprintId,
        code: 'product',
        name: 'Product',
        version: 7,
        status: 'published',
        views: { detail: { type: 'stack', children: [] } },
      },
      attributes: [{ id: 'attribute-id', code: 'title', value_type: 'string' }],
      table_path_attributes: [],
    };
    vi.mocked(api.getBlueprintRevision).mockResolvedValue(blueprint as never);
    vi.mocked(api.getCurrentBlueprint).mockResolvedValue(blueprint as never);

    renderPage();

    const runtimeScope = { blueprintId, blueprintVersion: 7 };
    await waitFor(() =>
      expect(outletRender).toHaveBeenCalledWith(
        expect.objectContaining({ outlet: 'entity_action', runtimeScope }),
      ),
    );
    expect(popoverOutletRender).toHaveBeenCalledWith(
      expect.objectContaining({
        outlet: 'entity_attribute_decoration',
        runtimeScope,
      }),
    );
    expect(drawerRender).toHaveBeenCalledWith(
      expect.objectContaining({ blueprintId, blueprintVersion: 7 }),
    );
  });

  it('shows a retryable error instead of a blank preview when contexts fail to load', async () => {
    vi.mocked(listContexts).mockRejectedValue(
      new Error('contexts unavailable'),
    );

    renderPage();

    expect(
      await screen.findByText('Unable to load this information'),
    ).toBeTruthy();
    expect(screen.queryByLabelText('Context')).toBeNull();

    screen.getByRole('button', { name: 'Try again' }).click();

    await waitFor(() => expect(listContexts).toHaveBeenCalledTimes(2));
  });
});
