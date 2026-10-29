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
import '../../i18n';
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
  kind: 'embedded',
  outlet: 'record_attribute_decoration',
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
  title: 'Example action',
  version: 1,
  invoke: null,
};

const renderOutlet = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const outlet = (
    <ExtensionPopoverOutlet
      context={{
        attribute_id: '44444444-4444-4444-8444-444444444444',
        blueprint_id: '22222222-2222-4222-8222-222222222222',
        blueprint_version: 1,
        record_id: '33333333-3333-4333-8333-333333333333',
      }}
      label="Extension actions"
      outlet="record_attribute_decoration"
    />
  );
  return render(
    <QueryClientProvider client={queryClient}>{outlet}</QueryClientProvider>,
  );
};

beforeEach(() => vi.clearAllMocks());

describe('ExtensionOutlet', () => {
  it('mounts file panels only with a bounded file reference context', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      {
        ...contribution,
        id: 'file-panel',
        kind: 'panel',
        outlet: 'file_panel',
      },
      {
        ...contribution,
        id: 'wrong-kind',
        kind: 'action',
        outlet: 'file_panel',
      },
    ]);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const context = {
      context_version: 1,
      file_id: '55555555-5555-4555-8555-555555555555',
      record_id: '33333333-3333-4333-8333-333333333333',
      attribute_id: '44444444-4444-4444-8444-444444444444',
      blueprint_id: '22222222-2222-4222-8222-222222222222',
      blueprint_version: 1,
    };
    const { rerender } = render(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet outlet="file_panel" context={context} />
      </QueryClientProvider>,
    );
    expect(await screen.findByText('file-panel')).toBeTruthy();
    expect(screen.queryByText('wrong-kind')).toBeNull();
    rerender(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet
          outlet="file_panel"
          context={{ ...context, filename: 'secret' }}
        />
      </QueryClientProvider>,
    );
    expect(screen.queryByText('file-panel')).toBeNull();
  });

  it('mounts only attribute panels with a strict record and revision context', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      {
        ...contribution,
        id: 'attribute-panel',
        kind: 'panel',
        outlet: 'record_attribute_panel',
      },
      {
        ...contribution,
        id: 'wrong-kind',
        kind: 'embedded',
        outlet: 'record_attribute_panel',
      },
    ]);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const context = {
      context_version: 1,
      record_id: '33333333-3333-4333-8333-333333333333',
      attribute_id: '44444444-4444-4444-8444-444444444444',
      blueprint_id: '22222222-2222-4222-8222-222222222222',
      blueprint_version: 1,
      context_id: null,
    };
    const { rerender } = render(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet outlet="record_attribute_panel" context={context} />
      </QueryClientProvider>,
    );
    expect(await screen.findByText('attribute-panel')).toBeTruthy();
    expect(screen.queryByText('wrong-kind')).toBeNull();
    rerender(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet
          outlet="record_attribute_panel"
          context={{ ...context, value: 'private' }}
        />
      </QueryClientProvider>,
    );
    expect(screen.queryByText('attribute-panel')).toBeNull();
  });

  it('mounts only read-only publish panels for a strict blueprint revision', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      {
        ...contribution,
        id: 'publish-check',
        kind: 'panel',
        outlet: 'blueprint_publish_check',
      },
      {
        ...contribution,
        id: 'wrong-kind',
        kind: 'action',
        outlet: 'blueprint_publish_check',
      },
    ]);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const context = {
      context_version: 1,
      blueprint_id: '22222222-2222-4222-8222-222222222222',
      blueprint_version: 1,
    };
    const { rerender } = render(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet outlet="blueprint_publish_check" context={context} />
      </QueryClientProvider>,
    );
    expect(await screen.findByText('publish-check')).toBeTruthy();
    expect(screen.queryByText('wrong-kind')).toBeNull();
    rerender(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet
          outlet="blueprint_publish_check"
          context={{ ...context, draft: true }}
        />
      </QueryClientProvider>,
    );
    expect(screen.queryByText('publish-check')).toBeNull();
  });

  it('renders only declared data-health panels in host-owned cards', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      {
        ...contribution,
        id: 'health-card',
        kind: 'panel',
        outlet: 'data_health_card',
      },
      {
        ...contribution,
        id: 'wrong-kind',
        kind: 'embedded',
        outlet: 'data_health_card',
      },
    ]);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const { rerender } = render(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet
          outlet="data_health_card"
          context={{ context_version: 1 }}
        />
      </QueryClientProvider>,
    );
    expect(await screen.findByText('health-card')).toBeTruthy();
    expect(
      screen.getByText('health-card').closest('.MuiPaper-root'),
    ).toBeTruthy();
    expect(screen.queryByText('wrong-kind')).toBeNull();
    rerender(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet
          outlet="data_health_card"
          context={{ context_version: 1, summary: { secret: 'private' } }}
        />
      </QueryClientProvider>,
    );
    expect(screen.queryByText('health-card')).toBeNull();
  });

  it('mounts only audit panels with an event ID and no event payload', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      {
        ...contribution,
        id: 'audit-panel',
        kind: 'panel',
        outlet: 'audit_event_panel',
      },
      {
        ...contribution,
        id: 'wrong-kind',
        kind: 'action',
        outlet: 'audit_event_panel',
      },
    ]);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const context = {
      context_version: 1,
      event_id: '22222222-2222-4222-8222-222222222222',
    };
    const { rerender } = render(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet outlet="audit_event_panel" context={context} />
      </QueryClientProvider>,
    );
    expect(await screen.findByText('audit-panel')).toBeTruthy();
    expect(screen.queryByText('wrong-kind')).toBeNull();
    rerender(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet
          outlet="audit_event_panel"
          context={{ ...context, metadata: { secret: 'private' } }}
        />
      </QueryClientProvider>,
    );
    expect(screen.queryByText('audit-panel')).toBeNull();
  });

  it('mounts only read-only blueprint panels with a strict revision context', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      {
        ...contribution,
        id: 'blueprint-panel',
        kind: 'panel',
        outlet: 'blueprint_panel',
      },
      {
        ...contribution,
        id: 'wrong-kind',
        kind: 'action',
        outlet: 'blueprint_panel',
      },
    ]);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const context = {
      context_version: 1,
      blueprint_id: '22222222-2222-4222-8222-222222222222',
      blueprint_version: 1,
    };
    const { rerender } = render(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet outlet="blueprint_panel" context={context} />
      </QueryClientProvider>,
    );
    expect(await screen.findByText('blueprint-panel')).toBeTruthy();
    expect(screen.queryByText('wrong-kind')).toBeNull();
    rerender(
      <QueryClientProvider client={queryClient}>
        <ExtensionOutlet
          outlet="blueprint_panel"
          context={{ ...context, unrelated: 'private' }}
        />
      </QueryClientProvider>,
    );
    expect(screen.queryByText('blueprint-panel')).toBeNull();
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
      screen.getByRole('button', { name: 'Extension navigation' }),
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
      expect(
        await screen.findByText(
          'No apps here yet. Apps appear when an enabled extension provides them.',
        ),
      ).toBeTruthy();
      expect(
        screen
          .getByRole('link', { name: 'Browse extensions' })
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
    expect(
      await screen.findByText(
        'No apps here yet. Apps appear when an enabled extension provides them.',
      ),
    ).toBeTruthy();
    expect(screen.queryByText('Browse extensions')).toBeNull();
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
