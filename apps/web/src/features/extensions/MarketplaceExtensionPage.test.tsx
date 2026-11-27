// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';
import '../../i18n';
import {
  extensionUpdates,
  installExtension,
  installedExtensions,
  registryDetails,
  upgradeExtension,
} from './managementApi';
import { MarketplaceExtensionPage } from './MarketplaceExtensionPage';

vi.mock('@tanstack/react-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@tanstack/react-router')>()),
  Link: () => null,
}));
vi.mock('../../components/RouterLink', () => ({
  RouterButton: ({ children }: { children: ReactNode }) => (
    <span>{children}</span>
  ),
}));
vi.mock('../auth/useCan', () => ({ useCan: () => true }));
vi.mock('./managementApi', () => ({
  extensionUpdates: vi.fn(),
  installExtension: vi.fn(),
  installedExtensions: vi.fn(),
  registryDetails: vi.fn(),
  upgradeExtension: vi.fn(),
}));

const release = (release_id: number, tag_name: string, name: string) => ({
  source: 'attricat/example',
  release_id,
  tag_name,
  name,
  published_at: null,
  asset: { id: release_id, name: 'example.tar.zst', download_url: 'https://x' },
});

const renderPage = (
  releases: ReturnType<typeof release>[],
  installedVersion?: string,
) => {
  vi.mocked(installedExtensions).mockResolvedValue(
    (installedVersion
      ? [
          {
            id: '00000000-0000-4000-8000-000000000001',
            extension_id: 'example',
            state: 'enabled',
            version: installedVersion,
            manifest: { name: 'Example' },
            source: `github:attricat/example@v${installedVersion}`,
          },
        ]
      : []) as never,
  );
  vi.mocked(extensionUpdates).mockResolvedValue(
    installedVersion && installedVersion !== '2.0.0'
      ? [
          {
            extension_id: 'example',
            installed_version: installedVersion,
            latest_version: '2.0.0',
            repository: 'github:attricat/example',
            registry_source: 'github:attricat/registry',
          },
        ]
      : [],
  );
  vi.mocked(registryDetails).mockResolvedValue({
    extension: {
      repository: 'github:attricat/example',
      name: 'Example',
      description: 'An example',
      registry_source: 'attricat/registry',
    },
    readme: '',
    releases,
  } as never);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MarketplaceExtensionPage owner="attricat" repository="example" />
    </QueryClientProvider>,
  );
};

describe('MarketplaceExtensionPage', () => {
  beforeEach(() => vi.clearAllMocks());

  it('upgrades an installed extension instead of installing it again', async () => {
    vi.mocked(upgradeExtension).mockResolvedValue({} as never);
    renderPage(
      [release(2, 'v2.0.0', 'Second'), release(1, 'v1.0.0', 'v1.0.0')],
      '1.0.0',
    );

    await screen.findByText('v1.0.0 is installed; v2.0.0 is available.');
    expect(screen.queryByRole('button', { name: 'Install' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Update' }));
    const dialog = await screen.findByRole('dialog');
    expect(upgradeExtension).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Upgrade' }));
    await waitFor(() =>
      expect(upgradeExtension).toHaveBeenCalledWith('example', {
        owner: 'attricat',
        repository: 'example',
        release_id: 2,
      }),
    );
  });

  it('says an up-to-date installation is installed', async () => {
    renderPage([release(2, 'v2.0.0', 'Second')], '2.0.0');

    await screen.findByText('v2.0.0 is installed.');
    expect(screen.queryByRole('button', { name: 'Update' })).toBeNull();
  });

  it('offers every release in one select and installs the selection', async () => {
    vi.mocked(installExtension).mockResolvedValue({} as never);
    renderPage([
      release(2, 'v2.0.0', 'Second'),
      release(1, 'v1.0.0', 'v1.0.0'),
    ]);

    const select = await screen.findByRole('combobox', {
      name: 'Requested release',
    });
    expect(select.textContent).toBe('Second (v2.0.0)');
    fireEvent.mouseDown(select);
    fireEvent.click(await screen.findByRole('option', { name: 'v1.0.0' }));
    const installButtons = screen.getAllByRole('button', { name: 'Install' });
    expect(installButtons).toHaveLength(1);
    fireEvent.click(installButtons[0]!);

    await waitFor(() =>
      expect(vi.mocked(installExtension).mock.calls[0]?.[0]).toEqual({
        owner: 'attricat',
        repository: 'example',
        release_id: 1,
      }),
    );
  });

  it('explains when no release can be installed', async () => {
    renderPage([]);

    await screen.findByText(
      'No reviewed stable releases are available to install.',
    );
    expect(screen.queryByRole('combobox')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Install' })).toBeNull();
  });
});
