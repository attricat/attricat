// @vitest-environment jsdom
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
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
  navigation_group: null,
  capabilities: [],
  configuration: null,
  extension_id: 'example.extension',
  extension_name: 'Example Extension',
  id: 'row-action',
  kind: 'action',
  outlet: 'explorer_row_action',
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
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

beforeEach(() => vi.clearAllMocks());

describe('ExtensionOutlet', () => {
  it('mounts only action contributions with a valid entity header context', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      { ...contribution, id: 'header-action', outlet: 'entity_header_action' },
      {
        ...contribution,
        id: 'wrong-kind',
        kind: 'panel',
        outlet: 'entity_header_action',
      },
    ]);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const context = {
      context_version: 1,
      entity_id: '33333333-3333-4333-8333-333333333333',
      blueprint_id: '22222222-2222-4222-8222-222222222222',
      blueprint_version: 1,
    };
    const { rerender } = render(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet outlet="entity_header_action" context={context} />
      </QueryClientProvider>,
    );
    expect(await screen.findByText('header-action')).toBeTruthy();
    expect(screen.queryByText('wrong-kind')).toBeNull();
    rerender(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet
          outlet="entity_header_action"
          context={{ ...context, unexpected: 'private' }}
        />
      </QueryClientProvider>,
    );
    expect(screen.queryByText('header-action')).toBeNull();
  });

  it.each([
    'entity_preview_panel',
    'entity_action',
    'entity_attribute_decoration',
  ] as const)('requests an explicit blueprint scope for %s', async (outlet) => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([]);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const props = {
      context: { entity_id: '33333333-3333-4333-8333-333333333333' },
      outlet,
      runtimeScope: {
        blueprintId: '22222222-2222-4222-8222-222222222222',
        blueprintVersion: 4,
      },
    };
    render(
      <QueryClientProvider client={queryClient}>
        {outlet === 'entity_attribute_decoration' ? (
          <ExtensionPopoverOutlet {...props} label="Decorations" />
        ) : (
          <ExtensionOutlet {...props} />
        )}
      </QueryClientProvider>,
    );
    await waitFor(() =>
      expect(getExtensionRuntime).toHaveBeenCalledWith(props.runtimeScope),
    );
  });

  it('keeps navigation grouped unless the host descriptor promotes it', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      {
        ...contribution,
        contribution_key: 'example.extension:promoted',
        id: 'promoted',
        kind: 'embedded',
        navigation_group: 'promoted',
        outlet: 'navigation',
      },
      {
        ...contribution,
        contribution_key: 'example.extension:grouped',
        id: 'grouped',
        kind: 'embedded',
        navigation_group: 'grouped',
        outlet: 'navigation',
      },
    ]);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet outlet="navigation" />
      </QueryClientProvider>,
    );
    expect(await screen.findByText('promoted')).toBeTruthy();
    expect(screen.queryByText('grouped')).toBeNull();
    await user.click(
      screen.getByRole('button', { name: 'extensions.groupedNavigation' }),
    );
    expect(await screen.findByText('grouped')).toBeTruthy();
  });

  it('renders host-owned navigation links to the declared route', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      {
        ...contribution,
        contribution_key: 'example.extension:workbench-nav',
        id: 'workbench-nav',
        kind: 'navigation',
        navigation_group: 'promoted',
        outlet: 'navigation',
        route: 'workbench',
        title: 'Workbench',
      },
    ]);
    const rootRoute = createRootRoute();
    const indexRoute = createRoute({
      getParentRoute: () => rootRoute,
      path: '/extensions/$extensionId/$contributionId',
      component: () => (
        <ExtensionOutlet navigationDisplay="all" outlet="navigation" />
      ),
    });
    const router = createRouter({
      history: createMemoryHistory({
        initialEntries: ['/extensions/example.extension/workbench'],
      }),
      routeTree: rootRoute.addChildren([indexRoute]),
    });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    );
    const link = await screen.findByRole('link', { name: 'Workbench' });
    expect(link.getAttribute('href')).toBe(
      '/extensions/example.extension/workbench',
    );
    expect(link.getAttribute('aria-current')).toBe('page');
  });

  it.each([{ runtime: [] }, { runtime: [contribution] }])(
    'shows a link to extensions when no apps are contributed ($runtime)',
    async ({ runtime }) => {
      vi.mocked(getExtensionRuntime).mockResolvedValue(runtime);
      const rootRoute = createRootRoute();
      const appsRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: '/extensions',
        component: () => (
          <ExtensionOutlet
            canBrowseExtensions
            navigationDisplay="all"
            outlet="navigation"
          />
        ),
      });
      const router = createRouter({
        history: createMemoryHistory({ initialEntries: ['/extensions'] }),
        routeTree: rootRoute.addChildren([appsRoute]),
      });
      const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
      });
      render(
        <QueryClientProvider client={queryClient}>
          <RouterProvider router={router} />
        </QueryClientProvider>,
      );
      expect(await screen.findByText('extensions.noApps')).toBeTruthy();
      expect(
        screen
          .getByRole('link', { name: 'extensions.browseExtensions' })
          .getAttribute('href'),
      ).toBe('/manage/extensions');
    },
  );

  it('does not offer extension management to users without access', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([]);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet navigationDisplay="all" outlet="navigation" />
      </QueryClientProvider>,
    );
    expect(await screen.findByText('extensions.noApps')).toBeTruthy();
    expect(screen.queryByText('extensions.browseExtensions')).toBeNull();
  });

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
    await user.click(
      screen.getByRole('button', { name: 'extensions.moreActions' }),
    );
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
    expect(getExtensionRuntime).toHaveBeenCalledWith(undefined);
    await waitFor(() => expect(frameMounts).toHaveBeenCalledOnce());

    await user.click(button);

    expect(frameMounts).toHaveBeenCalledOnce();
  });
});
