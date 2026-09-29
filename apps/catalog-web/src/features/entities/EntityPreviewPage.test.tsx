// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import { listContexts } from '../contexts/api';
import { EntityPreviewPage } from './EntityPreviewPage';

const {
  drawerRender,
  agentDrawerRender,
  outletRender,
  popoverOutletRender,
  navigate,
} = vi.hoisted(() => ({
  navigate: vi.fn(),
  drawerRender: vi.fn(),
  agentDrawerRender: vi.fn(),
  outletRender: vi.fn(),
  popoverOutletRender: vi.fn(),
}));

vi.mock('@tanstack/react-router', () => ({
  createLink: <T,>(component: T) => component,
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  useNavigate: () => navigate,
}));

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  duplicateEntity: vi.fn(),
  deleteEntity: vi.fn(),
  getBlueprintRevision: vi.fn(),
  getCurrentBlueprint: vi.fn(),
  getEntityPublications: vi.fn(),
  getResolvedEntityPreview: vi.fn(),
  publishEntity: vi.fn(),
  publishEntityAllChannels: vi.fn(),
  unpublishEntity: vi.fn(),
}));

vi.mock('../extensions/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../extensions/api')>()),
  getExtensionRuntime: vi.fn(),
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

vi.mock('./components/EntityAgentDrawer', () => ({
  EntityAgentDrawer: (props: unknown) => {
    agentDrawerRender(props);
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
    renderAttributePanel,
  }: {
    renderAttributeDecoration?: (attribute: {
      id: string;
      code: string;
    }) => React.ReactNode;
    renderAttributePanel?: (attribute: {
      id: string;
      code: string;
    }) => React.ReactNode;
  }) => (
    <>
      {renderAttributeDecoration?.({
        id: '44444444-4444-4444-8444-444444444444',
        code: 'title',
      })}
      {renderAttributePanel?.({
        id: '44444444-4444-4444-8444-444444444444',
        code: 'title',
      })}
    </>
  ),
}));

vi.mock('../auth/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../auth/api')>()),
  currentSession: vi.fn().mockResolvedValue(null),
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
    const { getExtensionRuntime } = await import('../extensions/api');
    vi.mocked(getExtensionRuntime).mockReset();
    vi.mocked(getExtensionRuntime).mockResolvedValue([]);
    const { currentSession } = await import('../auth/api');
    vi.mocked(currentSession).mockReset();
    vi.mocked(currentSession).mockResolvedValue(null);
    const api = await import('./api');
    vi.mocked(api.getEntityPublications).mockResolvedValue([]);
  });
  it('offers confirmed deletion only when permitted and navigates away afterwards', async () => {
    const { getExtensionRuntime } = await import('../extensions/api');
    outletRender.mockClear();
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      { outlet: 'entity_attribute_panel', kind: 'panel' },
    ] as never);
    const { currentSession } = await import('../auth/api');
    const api = await import('./api');
    const user = userEvent.setup();
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
        blueprint_id: '22222222-2222-4222-8222-222222222222',
        blueprint_version: 1,
      },
      values: {},
    } as never);
    vi.mocked(api.getBlueprintRevision).mockResolvedValue({
      blueprint: {
        id: '22222222-2222-4222-8222-222222222222',
        name: 'Product',
        code: 'product',
        version: 1,
        views: {},
      },
      attributes: [],
    } as never);
    vi.mocked(api.getCurrentBlueprint).mockResolvedValue({
      blueprint: { version: 1 },
    } as never);
    vi.mocked(currentSession).mockResolvedValueOnce({
      capabilities: { entities_delete: false },
    } as never);
    const { unmount } = renderPage();
    await screen.findByText('Product');
    await waitFor(() =>
      expect(outletRender).toHaveBeenCalledWith({
        outlet: 'entity_attribute_panel',
        context: {
          context_version: 1,
          entity_id: '00000000-0000-4000-8000-000000000001',
          attribute_id: '44444444-4444-4444-8444-444444444444',
          blueprint_id: '22222222-2222-4222-8222-222222222222',
          blueprint_version: 1,
          context_id: '33333333-3333-4333-8333-333333333333',
        },
        runtimeScope: {
          blueprintId: '22222222-2222-4222-8222-222222222222',
          blueprintVersion: 1,
        },
      }),
    );
    expect(screen.queryByRole('button', { name: 'Delete entity' })).toBeNull();
    unmount();

    vi.mocked(currentSession).mockResolvedValueOnce({
      capabilities: { entities_delete: true },
    } as never);
    vi.mocked(api.deleteEntity).mockResolvedValue(undefined);
    renderPage();
    await user.click(
      await screen.findByRole('button', { name: 'Delete entity' }),
    );
    expect(api.deleteEntity).not.toHaveBeenCalled();
    await user.click(
      within(screen.getByRole('dialog')).getByRole('button', {
        name: 'Delete entity',
      }),
    );
    await waitFor(() =>
      expect(api.deleteEntity).toHaveBeenCalledWith(
        '00000000-0000-4000-8000-000000000001',
      ),
    );
    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/' }));
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
        is_sample: true,
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

    expect(await screen.findByText('Sample')).toBeTruthy();
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
    await userEvent
      .setup()
      .click(screen.getByRole('button', { name: 'Ask about this entity' }));
    expect(agentDrawerRender).toHaveBeenCalledWith(
      expect.objectContaining({
        entityId: '00000000-0000-4000-8000-000000000001',
        contextId: '33333333-3333-4333-8333-333333333333',
        open: true,
      }),
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
