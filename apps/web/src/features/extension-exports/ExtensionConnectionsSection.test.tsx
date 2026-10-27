// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import { connectionAction, deleteConnection, listConnections } from './api';
import { ExtensionConnectionsSection } from './ExtensionConnectionsSection';
import type { ExtensionConnection, ManifestExports } from './schemas';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  connectionAction: vi.fn(),
  deleteConnection: vi.fn(),
  listConnections: vi.fn(),
}));

const extensionId = 'acme.exports';
const manifest = {
  server: {
    connections: [
      { id: 'shop', title: 'Shop', grant_types: ['client_credentials'] },
    ],
    exports: [],
  },
  ui: [],
} as ManifestExports;
const connection = {
  id: '00000000-0000-4000-8000-000000000001',
  extension_id: extensionId,
  connection: 'shop',
  kind: 'shop',
  display_name: 'Main shop',
  grant_type: 'client_credentials',
  host_permission_id: 'shop',
  token_url: 'https://shop.example/token',
  device_authorization_url: null,
  client_id: 'client',
  scope: null,
  parameters: {},
  status: 'active',
  last_error: null,
  last_refreshed_at: null,
  access_token_expires_at: null,
  device_user_code: null,
  device_verification_uri: null,
  device_expires_at: null,
  authorization_url: null,
  authorization_expires_at: null,
  has_client_secret: true,
  has_refresh_token: false,
} as ExtensionConnection;

const renderSection = () =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ExtensionConnectionsSection
        canManage
        extensionId={extensionId}
        manifest={manifest}
      />
    </QueryClientProvider>,
  );

describe('ExtensionConnectionsSection', () => {
  it('asks before deleting a connection', async () => {
    vi.mocked(listConnections).mockResolvedValue([connection]);
    vi.mocked(deleteConnection).mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByRole('button', { name: 'Delete' }));
    expect(deleteConnection).not.toHaveBeenCalled();
    const dialog = screen.getByRole('dialog', { name: 'Delete Main shop?' });
    await user.click(
      Array.from(dialog.querySelectorAll('button')).find(
        (button) => button.textContent === 'Delete',
      )!,
    );
    await waitFor(() =>
      expect(deleteConnection).toHaveBeenCalledWith(extensionId, 'shop'),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('clears an earlier failure when another action starts', async () => {
    vi.mocked(listConnections).mockResolvedValue([connection]);
    vi.mocked(connectionAction).mockRejectedValue(
      new ApiRequestError(502, 'Provider unreachable', 'connection_failed'),
    );
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByRole('button', { name: 'Test' }));
    expect(await screen.findByRole('alert')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Delete' }));
    expect(screen.queryByRole('alert')).toBeNull();
  });
});
