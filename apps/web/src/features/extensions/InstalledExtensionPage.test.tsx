// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { InstalledExtensionPage } from './InstalledExtensionPage';
import {
  extensionDetail,
  extensionSecrets,
  extensionUpdates,
  registryDetails,
  removeExtension,
  upgradeExtension,
  upgradeExtensionFromArchive,
} from './managementApi';
import {
  installedExtensionTabs,
  type InstalledExtensionTab,
} from './constants';

vi.mock('../auth/useCan', () => ({ useCan: () => true }));
vi.mock('./managementApi', () => ({
  extensionDetail: vi.fn(),
  extensionSecrets: vi.fn(),
  extensionUpdates: vi.fn(),
  lifecycleExtension: vi.fn(),
  registryDetails: vi.fn(),
  removeExtension: vi.fn(),
  upgradeExtension: vi.fn(),
  upgradeExtensionFromArchive: vi.fn(),
}));
vi.mock('../extension-exports/ExtensionBudgetsSection', () => ({
  ExtensionBudgetsSection: () => null,
}));
vi.mock('../extension-exports/ExtensionExportStatusSection', () => ({
  ExtensionExportStatusSection: () => <p>export status</p>,
}));
vi.mock('../extension-exports/ExtensionConnectionsSection', () => ({
  ExtensionConnectionsSection: () => <p>connections</p>,
}));
vi.mock('../extension-webhooks/ExtensionWebhooksSection', () => ({
  ExtensionWebhooksSection: () => <p>webhooks</p>,
}));
vi.mock('./ExtensionSecretsSection', () => ({
  ExtensionSecretsSection: () => <p>secrets</p>,
}));

const registrySource = 'github:attricat/example@v1.0.0';

const release = (release_id: number, tag_name: string) => ({
  source: 'github:attricat/example',
  release_id,
  tag_name,
  name: tag_name,
  published_at: null,
  asset: { id: release_id, name: 'example.tar.zst', download_url: 'https://x' },
});

const detail = (manifest: Record<string, unknown>, source = 'sideload') => ({
  grants: [],
  lifecycle: [],
  installation: {
    id: '00000000-0000-4000-8000-000000000001',
    extension_id: 'example',
    installed_release_id: '00000000-0000-4000-8000-000000000002',
    state: 'disabled',
    configuration: {},
    configuration_version: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    version: '1.0.0',
    manifest: { name: 'Example', description: 'Does things', ...manifest },
    manifest_unreadable: Boolean(manifest.unreadable),
    manifest_sha256: 'abc123',
    source,
  },
});

const renderPage = async (
  manifest: Record<string, unknown>,
  tab?: InstalledExtensionTab,
  source?: string,
) => {
  vi.mocked(extensionDetail).mockResolvedValue(
    detail(manifest, source) as never,
  );
  const root = createRootRoute();
  const page = createRoute({
    getParentRoute: () => root,
    path: '/manage/extensions/$extensionId',
    component: () => <InstalledExtensionPage extensionId="example" tab={tab} />,
  });
  const list = createRoute({
    getParentRoute: () => root,
    path: '/manage/extensions',
  });
  const installed = createRoute({
    getParentRoute: () => root,
    path: '/manage/extensions/installed',
    component: () => <p>Installed extensions</p>,
  });
  const marketplace = createRoute({
    getParentRoute: () => root,
    path: '/manage/extensions/$owner/$repository',
  });
  const router = createRouter({
    history: createMemoryHistory({
      initialEntries: ['/manage/extensions/example'],
    }),
    routeTree: root.addChildren([page, list, installed, marketplace]),
  });
  render(
    <QueryClientProvider client={new QueryClient()}>
      <RouterProvider router={router as never} />
    </QueryClientProvider>,
  );
  await screen.findByRole('tab', { name: 'Overview' });
};

const tabNames = () => screen.getAllByRole('tab').map((tab) => tab.textContent);

describe('InstalledExtensionPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(extensionUpdates).mockResolvedValue([]);
  });

  it('leads the overview with the update and upgrades only after confirming', async () => {
    vi.mocked(extensionUpdates).mockResolvedValue([
      {
        extension_id: 'example',
        installed_version: '1.0.0',
        latest_version: '1.2.0',
        repository: 'github:attricat/example',
        registry_source: 'github:attricat/registry',
      },
    ]);
    vi.mocked(registryDetails).mockResolvedValue({
      extension: {},
      readme: '',
      releases: [release(3, 'v1.2.0'), release(2, 'v1.1.0')],
    } as never);
    vi.mocked(upgradeExtension).mockResolvedValue(undefined as never);
    await renderPage({}, undefined, registrySource);

    const card = await screen.findByRole('heading', {
      name: 'Update available: 1.2.0',
    });
    const details = screen.getByRole('heading', { name: 'Details' });
    expect(
      card.compareDocumentPosition(details) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      screen
        .getByRole('link', { name: 'View in marketplace' })
        .getAttribute('href'),
    ).toBe('/manage/extensions/attricat/example');

    fireEvent.click(screen.getByRole('button', { name: 'Upgrade to v1.2.0' }));
    const dialog = await screen.findByRole('dialog');
    expect(dialog.textContent).toContain('from v1.0.0 to v1.2.0');
    expect(upgradeExtension).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Upgrade' }));
    await waitFor(() =>
      expect(upgradeExtension).toHaveBeenCalledWith('example', {
        owner: 'attricat',
        repository: 'example',
        release_id: 3,
      }),
    );
  });

  it('offers no update or marketplace link for a side-loaded extension', async () => {
    await renderPage({});
    expect(
      screen.queryByRole('link', { name: 'View in marketplace' }),
    ).toBeNull();
    expect(registryDetails).not.toHaveBeenCalled();
  });

  it('says when this version cannot read the stored manifest', async () => {
    await renderPage({ unreadable: true });
    expect(screen.getByRole('alert').textContent).toContain(
      "can't read this extension's manifest",
    );
  });

  it('offers export and connection tabs only to extensions declaring them', async () => {
    await renderPage({});
    expect(tabNames()).toEqual([
      'Overview',
      'Configuration',
      'Permissions',
      'Lifecycle',
    ]);
  });

  it('adds the export and connection tabs when the manifest declares them', async () => {
    await renderPage({
      server: {
        connections: [{ id: 'crm', title: 'CRM', grant_types: [] }],
        exports: [{ id: 'push', title: 'Push' }],
      },
    });
    expect(tabNames()).toEqual([
      'Overview',
      'Configuration',
      'Permissions',
      'Exports',
      'Connections',
      'Lifecycle',
    ]);
  });

  it('adds the webhooks tab when the manifest declares webhooks', async () => {
    await renderPage({
      server: {
        webhooks: [{ id: 'orders', title: 'Orders', max_body_bytes: 1024 }],
      },
    });
    expect(tabNames()).toEqual([
      'Overview',
      'Configuration',
      'Permissions',
      'Webhooks',
      'Lifecycle',
    ]);
  });

  it('adds the secrets tab and warns about required secrets that are not set', async () => {
    vi.mocked(extensionSecrets).mockResolvedValue([
      { name: 'api-key', declared: true, required: true, set: false },
      { name: 'sandbox', declared: true, required: false, set: false },
    ]);
    await renderPage({
      optional_permissions: ['secrets.read'],
      server: { secrets: [{ name: 'api-key', title: 'API key' }] },
    });
    expect(tabNames()).toEqual([
      'Overview',
      'Configuration',
      'Permissions',
      'Secrets',
      'Lifecycle',
    ]);
    expect(
      await screen.findByText('1 required secret is not set.'),
    ).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Set secrets' })).toBeTruthy();
    expect(extensionSecrets).toHaveBeenCalledWith('example');
  });

  it('falls back to the overview for a tab this extension does not have', async () => {
    await renderPage({}, installedExtensionTabs.exports);
    expect(
      screen
        .getByRole('tab', { name: 'Overview' })
        .getAttribute('aria-selected'),
    ).toBe('true');
    expect(screen.getByRole('heading', { name: 'Details' })).toBeTruthy();
  });

  it('keeps the other panels mounted but hidden', async () => {
    await renderPage({}, installedExtensionTabs.permissions);
    expect(screen.queryByRole('heading', { name: 'Details' })).toBeNull();
    expect(screen.getByText('Details')).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Permissions' })).toBeTruthy();
  });

  it('points the overview at permissions still waiting to be granted', async () => {
    await renderPage({ permissions: ['attricat.read', 'attricat.write'] });
    expect(
      screen.getByText('2 requested permissions are waiting to be granted.'),
    ).toBeTruthy();
    expect(
      screen.getByRole('link', { name: 'Review permissions' }),
    ).toBeTruthy();
  });

  it('upgrades from an archive only after the person confirms', async () => {
    vi.mocked(upgradeExtensionFromArchive).mockResolvedValue(
      undefined as never,
    );
    await renderPage({});
    const archive = new File(['x'], 'example.tar.zst');
    const input = screen
      .getByText('Upgrade from archive')
      .querySelector('input');
    fireEvent.change(input as HTMLInputElement, {
      target: { files: [archive] },
    });
    const dialog = await screen.findByRole('dialog');
    expect(dialog.textContent).toContain('example.tar.zst');
    expect(dialog.textContent).toContain(
      'Permissions and configuration carry over',
    );
    expect(upgradeExtensionFromArchive).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Upgrade' }));
    await waitFor(() =>
      expect(upgradeExtensionFromArchive).toHaveBeenCalledWith(
        'example',
        archive,
      ),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('leaves the page once the extension is removed', async () => {
    vi.mocked(removeExtension).mockResolvedValue(undefined as never);
    await renderPage({});
    const reads = vi.mocked(extensionDetail).mock.calls.length;

    fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
    expect(removeExtension).not.toHaveBeenCalled();
    fireEvent.click(
      within(await screen.findByRole('dialog')).getByRole('button', {
        name: 'Remove',
      }),
    );

    await screen.findByText('Installed extensions');
    expect(removeExtension).toHaveBeenCalledWith('example');
    expect(extensionDetail).toHaveBeenCalledTimes(reads);
  });
});
