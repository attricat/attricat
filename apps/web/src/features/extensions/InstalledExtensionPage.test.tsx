// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { InstalledExtensionPage } from './InstalledExtensionPage';
import { extensionDetail, upgradeExtensionFromArchive } from './managementApi';
import {
  installedExtensionTabs,
  type InstalledExtensionTab,
} from './constants';

vi.mock('../auth/useCan', () => ({ useCan: () => true }));
vi.mock('./managementApi', () => ({
  extensionDetail: vi.fn(),
  lifecycleExtension: vi.fn(),
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

const detail = (manifest: Record<string, unknown>) => ({
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
    manifest_sha256: 'abc123',
    source: 'sideload',
  },
});

const renderPage = async (
  manifest: Record<string, unknown>,
  tab?: InstalledExtensionTab,
) => {
  vi.mocked(extensionDetail).mockResolvedValue(detail(manifest) as never);
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
  const router = createRouter({
    history: createMemoryHistory({
      initialEntries: ['/manage/extensions/example'],
    }),
    routeTree: root.addChildren([page, list]),
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
  beforeEach(() => vi.clearAllMocks());

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
    expect(dialog.textContent).toContain('Permissions and configuration carry over');
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
});
