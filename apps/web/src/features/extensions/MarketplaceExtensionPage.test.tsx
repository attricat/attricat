// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { installExtension, registryDetails } from './managementApi';
import { MarketplaceExtensionPage } from './MarketplaceExtensionPage';

vi.mock('@tanstack/react-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@tanstack/react-router')>()),
  Link: () => null,
}));
vi.mock('../auth/useCan', () => ({ useCan: () => true }));
vi.mock('./managementApi', () => ({
  installExtension: vi.fn(),
  registryDetails: vi.fn(),
}));

const release = (release_id: number, tag_name: string, name: string) => ({
  source: 'attricat/example',
  release_id,
  tag_name,
  name,
  published_at: null,
  asset: { id: release_id, name: 'example.tar.zst', download_url: 'https://x' },
});

const renderPage = (releases: ReturnType<typeof release>[]) => {
  vi.mocked(registryDetails).mockResolvedValue({
    extension: {
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

    await screen.findByText('No installable stable releases were found.');
    expect(screen.queryByRole('combobox')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Install' })).toBeNull();
  });
});
