// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { ExtensionSecretsPage } from './ExtensionSecretsPage';
import {
  deleteWorkspaceExtensionSecret,
  putWorkspaceExtensionSecret,
  workspaceExtensionSecrets,
} from './managementApi';

vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./managementApi', () => ({
  deleteWorkspaceExtensionSecret: vi.fn(),
  putWorkspaceExtensionSecret: vi.fn(),
  workspaceExtensionSecrets: vi.fn(),
}));

const managerSession = { capabilities: { extensions_manage: true } };
const secret = {
  name: 'destination-token',
  created_at: '2026-11-20T10:00:00Z',
  updated_at: '2026-11-21T10:00:00Z',
};

const renderPage = (session: object = managerSession) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  vi.mocked(currentSession).mockResolvedValue(session as never);
  client.setQueryData(authQueryKeys.session(), session);
  render(
    <QueryClientProvider client={client}>
      <ExtensionSecretsPage />
    </QueryClientProvider>,
  );
  return client;
};

describe('ExtensionSecretsPage', () => {
  beforeEach(() => {
    vi.mocked(workspaceExtensionSecrets).mockReset();
    vi.mocked(putWorkspaceExtensionSecret).mockReset();
    vi.mocked(deleteWorkspaceExtensionSecret).mockReset();
  });

  it('lists secret names without values', async () => {
    vi.mocked(workspaceExtensionSecrets).mockResolvedValue([secret]);
    renderPage();
    const table = await screen.findByRole('table', { name: 'Secrets' });
    expect(within(table).getByText('destination-token')).toBeTruthy();
    expect(
      within(table).getByRole('button', {
        name: 'Replace value of destination-token',
      }),
    ).toBeTruthy();
  });

  it('adds a secret under a valid, unused name', async () => {
    vi.mocked(workspaceExtensionSecrets).mockResolvedValue([secret]);
    vi.mocked(putWorkspaceExtensionSecret).mockResolvedValue(undefined);
    const user = userEvent.setup();
    const client = renderPage();
    await screen.findByRole('table', { name: 'Secrets' });

    await user.click(screen.getByRole('button', { name: 'Add secret' }));
    const dialog = await screen.findByRole('dialog');
    const name = within(dialog).getByRole('textbox', { name: /Name/ });
    const value = within(dialog).getByLabelText(/Value/);
    const submit = within(dialog).getByRole('button', { name: 'Add secret' });

    await user.type(name, 'bad name');
    await user.type(value, 's3cret');
    await user.click(submit);
    expect(
      await within(dialog).findByText(/letters, digits, dots, hyphens/),
    ).toBeTruthy();

    await user.clear(name);
    await user.type(name, 'destination-token');
    await user.click(submit);
    expect(await within(dialog).findByText(/already exists/)).toBeTruthy();
    expect(putWorkspaceExtensionSecret).not.toHaveBeenCalled();

    await user.clear(name);
    await user.type(name, 'shop.api-key');
    await user.click(submit);
    await waitFor(() =>
      expect(vi.mocked(putWorkspaceExtensionSecret).mock.calls[0]?.[0]).toEqual(
        { name: 'shop.api-key', value: 's3cret' },
      ),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    // The write-only value does not stay in the query client's cache.
    await waitFor(() =>
      expect(
        JSON.stringify(
          client
            .getMutationCache()
            .getAll()
            .map((mutation) => mutation.state.variables),
        ),
      ).not.toContain('s3cret'),
    );
  });

  it('replaces the value of an existing secret', async () => {
    vi.mocked(workspaceExtensionSecrets).mockResolvedValue([secret]);
    vi.mocked(putWorkspaceExtensionSecret).mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderPage();
    await user.click(
      await screen.findByRole('button', {
        name: 'Replace value of destination-token',
      }),
    );
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).queryByRole('textbox', { name: /Name/ })).toBeNull();
    await user.type(within(dialog).getByLabelText(/New value/), 'rotated');
    await user.click(
      within(dialog).getByRole('button', { name: 'Replace value' }),
    );
    await waitFor(() =>
      expect(vi.mocked(putWorkspaceExtensionSecret).mock.calls[0]?.[0]).toEqual(
        { name: 'destination-token', value: 'rotated' },
      ),
    );
  });

  it('asks before deleting a secret', async () => {
    vi.mocked(workspaceExtensionSecrets).mockResolvedValue([secret]);
    vi.mocked(deleteWorkspaceExtensionSecret).mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderPage();
    await user.click(
      await screen.findByRole('button', { name: 'Delete destination-token' }),
    );
    const dialog = await screen.findByRole('dialog', {
      name: 'Delete secret destination-token?',
    });
    expect(deleteWorkspaceExtensionSecret).not.toHaveBeenCalled();
    await user.click(
      within(dialog).getByRole('button', { name: 'Delete secret' }),
    );
    await waitFor(() =>
      expect(vi.mocked(deleteWorkspaceExtensionSecret).mock.calls[0]?.[0]).toBe(
        'destination-token',
      ),
    );
  });

  it('tells members without extension management why it is unavailable', () => {
    renderPage({ capabilities: { extensions_manage: false } });
    expect(screen.getByRole('alert')).toBeTruthy();
    expect(workspaceExtensionSecrets).not.toHaveBeenCalled();
  });
});
