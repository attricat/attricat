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
import { ExtensionsInstalledPage } from './ExtensionsInstalledPage';
import {
  extensionUpdates,
  installedExtensions,
  registryDetails,
  upgradeExtension,
} from './managementApi';

vi.mock('../auth/useCan', () => ({ useCan: () => true }));
vi.mock('./managementApi', () => ({
  extensionUpdates: vi.fn(),
  installedExtensions: vi.fn(),
  registryDetails: vi.fn(),
  upgradeExtension: vi.fn(),
}));

const installation = (extension_id: string, source: string) => ({
  id: `00000000-0000-4000-8000-00000000000${extension_id.length}`,
  extension_id,
  state: 'enabled',
  version: '1.0.0',
  manifest: { name: extension_id },
  source,
});

const renderPage = async () => {
  const root = createRootRoute();
  const routes = [
    '/manage/extensions/installed',
    '/manage/extensions/marketplace',
    '/manage/extensions/$extensionId',
    '/manage/extensions/$owner/$repository',
  ].map((path) =>
    createRoute({
      getParentRoute: () => root,
      path,
      component:
        path === '/manage/extensions/installed'
          ? ExtensionsInstalledPage
          : undefined,
    }),
  );
  const router = createRouter({
    history: createMemoryHistory({
      initialEntries: ['/manage/extensions/installed'],
    }),
    routeTree: root.addChildren(routes),
  });
  render(
    <QueryClientProvider client={new QueryClient()}>
      <RouterProvider router={router as never} />
    </QueryClientProvider>,
  );
  await screen.findByRole('heading', { name: 'Installed' });
};

describe('ExtensionsInstalledPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(extensionUpdates).mockResolvedValue([]);
  });

  it('points an empty list at the marketplace', async () => {
    vi.mocked(installedExtensions).mockResolvedValue([]);
    await renderPage();

    const browse = await screen.findByRole('link', {
      name: 'Browse marketplace',
    });
    expect(browse.getAttribute('href')).toBe('/manage/extensions/marketplace');
  });

  it('links a registry extension to its marketplace page', async () => {
    vi.mocked(installedExtensions).mockResolvedValue([
      installation('registry', 'github:attricat/example@v1.0.0'),
      installation('sideloaded', 'sideload'),
    ] as never);
    await renderPage();

    const links = await screen.findAllByRole('link', {
      name: 'View in marketplace',
    });
    expect(links.map((link) => link.getAttribute('href'))).toEqual([
      '/manage/extensions/attricat/example',
    ]);
  });

  it('updates from the list only after the person confirms', async () => {
    vi.mocked(installedExtensions).mockResolvedValue([
      installation('registry', 'github:attricat/example@v1.0.0'),
    ] as never);
    vi.mocked(extensionUpdates).mockResolvedValue([
      {
        extension_id: 'registry',
        installed_version: '1.0.0',
        latest_version: '1.1.0',
        repository: 'github:attricat/example',
        registry_source: 'github:attricat/registry',
      },
    ]);
    vi.mocked(registryDetails).mockResolvedValue({
      extension: {},
      readme: '',
      releases: [
        {
          source: 'github:attricat/example',
          release_id: 7,
          tag_name: 'v1.1.0',
          name: 'v1.1.0',
          published_at: null,
          asset: { id: 7, name: 'example.tar.zst', download_url: 'https://x' },
        },
      ],
    } as never);
    vi.mocked(upgradeExtension).mockResolvedValue({} as never);
    await renderPage();

    fireEvent.click(
      await screen.findByRole('button', { name: 'Update registry' }),
    );
    const dialog = await screen.findByRole('dialog');
    expect(dialog.textContent).toContain('from v1.0.0 to v1.1.0');
    const confirm = within(dialog).getByRole('button', { name: 'Upgrade' });
    await waitFor(() => expect(confirm.hasAttribute('disabled')).toBe(false));
    expect(upgradeExtension).not.toHaveBeenCalled();
    fireEvent.click(confirm);
    await waitFor(() =>
      expect(upgradeExtension).toHaveBeenCalledWith('registry', {
        owner: 'attricat',
        repository: 'example',
        release_id: 7,
      }),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('says when the registry has no release for the update yet', async () => {
    vi.mocked(installedExtensions).mockResolvedValue([
      installation('registry', 'github:attricat/example@v1.0.0'),
    ] as never);
    vi.mocked(extensionUpdates).mockResolvedValue([
      {
        extension_id: 'registry',
        installed_version: '1.0.0',
        latest_version: '1.1.0',
        repository: 'github:attricat/example',
        registry_source: 'github:attricat/registry',
      },
    ]);
    vi.mocked(registryDetails).mockResolvedValue({
      extension: {},
      readme: '',
      releases: [],
    } as never);
    await renderPage();

    fireEvent.click(
      await screen.findByRole('button', { name: 'Update registry' }),
    );
    const dialog = await screen.findByRole('dialog');
    await within(dialog).findByText(/has no release for v1\.1\.0 yet/);
    expect(
      within(dialog)
        .getByRole('button', { name: 'Upgrade' })
        .hasAttribute('disabled'),
    ).toBe(true);
  });
});
