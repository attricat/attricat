// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ExtensionSecretsSection } from './ExtensionSecretsSection';
import {
  deleteExtensionSecret,
  extensionSecrets,
  putExtensionSecret,
} from './managementApi';
import type { ExtensionSecret } from './schemas';

vi.mock('./managementApi', () => ({
  deleteExtensionSecret: vi.fn(),
  extensionSecrets: vi.fn(),
  putExtensionSecret: vi.fn(),
}));

const extensionId = 'acme.shop';
const required: ExtensionSecret = {
  name: 'api-key',
  declared: true,
  title: 'API key',
  description: 'Issued in the shop admin.',
  required: true,
  set: false,
};
const undeclared: ExtensionSecret = {
  name: 'destination-token',
  declared: false,
  required: false,
  set: true,
  created_at: '2026-11-20T10:00:00Z',
  updated_at: '2026-11-21T10:00:00Z',
};

const renderSection = (canManage = true) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <ExtensionSecretsSection
        canManage={canManage}
        extensionId={extensionId}
      />
    </QueryClientProvider>,
  );
};

describe('ExtensionSecretsSection', () => {
  beforeEach(() => {
    vi.mocked(extensionSecrets).mockReset();
    vi.mocked(putExtensionSecret).mockReset();
    vi.mocked(deleteExtensionSecret).mockReset();
    vi.mocked(extensionSecrets).mockResolvedValue([required, undeclared]);
  });

  it('lists declared and set secrets by name with their status', async () => {
    renderSection();
    const table = await screen.findByRole('table', { name: 'Secrets' });
    expect(extensionSecrets).toHaveBeenCalledWith(extensionId);
    expect(within(table).getByText('API key')).toBeTruthy();
    expect(within(table).getByText('Issued in the shop admin.')).toBeTruthy();
    expect(within(table).getByText('Required, not set')).toBeTruthy();
    expect(within(table).getByText('Set')).toBeTruthy();
    expect(
      within(table).getByRole('button', { name: 'Set value of api-key' }),
    ).toBeTruthy();
    expect(
      within(table).getByRole('button', {
        name: 'Replace value of destination-token',
      }),
    ).toBeTruthy();
  });

  it('sets the value of a declared secret under its own name', async () => {
    vi.mocked(putExtensionSecret).mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderSection();
    await user.click(
      await screen.findByRole('button', { name: 'Set value of api-key' }),
    );
    const dialog = await screen.findByRole('dialog', {
      name: 'Set value of api-key',
    });
    expect(within(dialog).queryByRole('textbox', { name: /Name/ })).toBeNull();
    await user.type(within(dialog).getByLabelText(/Value/), 's3cret');
    await user.click(within(dialog).getByRole('button', { name: 'Set value' }));
    await waitFor(() =>
      expect(vi.mocked(putExtensionSecret).mock.calls[0]?.[0]).toEqual({
        extensionId,
        name: 'api-key',
        value: 's3cret',
      }),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('adds a secret under a valid name the extension does not use yet', async () => {
    vi.mocked(putExtensionSecret).mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderSection();
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

    // A declared name is taken even before it has a value.
    await user.clear(name);
    await user.type(name, 'api-key');
    await user.click(submit);
    expect(
      await within(dialog).findByText(/already has a secret/),
    ).toBeTruthy();
    expect(putExtensionSecret).not.toHaveBeenCalled();

    await user.clear(name);
    await user.type(name, 'shop.api-key');
    await user.click(submit);
    await waitFor(() =>
      expect(vi.mocked(putExtensionSecret).mock.calls[0]?.[0]).toEqual({
        extensionId,
        name: 'shop.api-key',
        value: 's3cret',
      }),
    );
  });

  it('replaces the value of a set secret', async () => {
    vi.mocked(putExtensionSecret).mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderSection();
    await user.click(
      await screen.findByRole('button', {
        name: 'Replace value of destination-token',
      }),
    );
    const dialog = await screen.findByRole('dialog');
    await user.type(within(dialog).getByLabelText(/New value/), 'rotated');
    await user.click(
      within(dialog).getByRole('button', { name: 'Replace value' }),
    );
    await waitFor(() =>
      expect(vi.mocked(putExtensionSecret).mock.calls[0]?.[0]).toEqual({
        extensionId,
        name: 'destination-token',
        value: 'rotated',
      }),
    );
  });

  it('asks before deleting a secret', async () => {
    vi.mocked(deleteExtensionSecret).mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderSection();
    await user.click(
      await screen.findByRole('button', { name: 'Delete destination-token' }),
    );
    const dialog = await screen.findByRole('dialog', {
      name: 'Delete secret destination-token?',
    });
    expect(deleteExtensionSecret).not.toHaveBeenCalled();
    await user.click(
      within(dialog).getByRole('button', { name: 'Delete secret' }),
    );
    await waitFor(() =>
      expect(vi.mocked(deleteExtensionSecret).mock.calls[0]?.[0]).toEqual({
        extensionId,
        name: 'destination-token',
      }),
    );
  });
});
