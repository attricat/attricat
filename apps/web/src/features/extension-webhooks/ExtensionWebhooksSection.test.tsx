// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import {
  createWebhookEndpoint,
  deleteWebhookEndpoint,
  listWebhookEndpoints,
  rotateWebhookEndpoint,
} from './api';
import { webhookEndpointResource } from './constants';
import { ExtensionWebhooksSection } from './ExtensionWebhooksSection';
import type {
  IssuedWebhookEndpoint,
  ManifestWebhook,
  WebhookEndpoint,
} from './schemas';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  createWebhookEndpoint: vi.fn(),
  deleteWebhookEndpoint: vi.fn(),
  listWebhookEndpoints: vi.fn(),
  rotateWebhookEndpoint: vi.fn(),
}));

const extensionId = 'acme.orders';
const webhooks: ManifestWebhook[] = [{ id: 'orders', title: 'Order events' }];
const endpoint: WebhookEndpoint = {
  id: '00000000-0000-4000-8000-000000000001',
  extension_id: extensionId,
  webhook: 'orders',
  endpoint: 'main-shop',
  created_at: '2026-11-01T10:00:00Z',
  token_rotated_at: null,
  last_request_at: '2026-11-20T10:00:00Z',
  last_reply_status: 202,
};
const issued = (
  overrides: Partial<IssuedWebhookEndpoint> = {},
): IssuedWebhookEndpoint => ({
  ...endpoint,
  path: '/api/public/extension-webhooks/secret-token',
  url: 'https://attricat.example/api/public/extension-webhooks/secret-token',
  ...overrides,
});

const renderSection = ({
  canManage = true,
  declared = webhooks,
}: { canManage?: boolean; declared?: ManifestWebhook[] } = {}) =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ExtensionWebhooksSection
        canManage={canManage}
        extensionId={extensionId}
        webhooks={declared}
      />
    </QueryClientProvider>,
  );

const dialogButton = (dialog: HTMLElement, name: string) =>
  within(dialog).getByRole('button', { name });

describe('ExtensionWebhooksSection', () => {
  beforeEach(() => vi.clearAllMocks());

  it('lists endpoints with their webhook and last reply', async () => {
    vi.mocked(listWebhookEndpoints).mockResolvedValue([
      endpoint,
      {
        ...endpoint,
        id: '00000000-0000-4000-8000-000000000002',
        endpoint: 'legacy',
        webhook: 'retired',
        last_request_at: null,
        last_reply_status: null,
      },
    ]);
    renderSection();

    const row = (await screen.findByText('main-shop')).closest('tr')!;
    expect(within(row).getByText('Order events')).toBeTruthy();
    expect(within(row).getByText('202')).toBeTruthy();
    const legacy = screen.getByText('legacy').closest('tr')!;
    // A webhook the release no longer declares is named by its ID.
    expect(within(legacy).getByText('retired')).toBeTruthy();
    expect(within(legacy).getByText('No requests yet')).toBeTruthy();
  });

  it('explains webhook endpoints when there are none', async () => {
    vi.mocked(listWebhookEndpoints).mockResolvedValue([]);
    renderSection();

    expect(await screen.findByText('No webhook endpoints')).toBeTruthy();
    expect(
      screen.getByText(/let external systems push events to this extension/),
    ).toBeTruthy();
  });

  it('offers management actions only to managers', async () => {
    vi.mocked(listWebhookEndpoints).mockResolvedValue([endpoint]);
    renderSection({ canManage: false });

    await screen.findByText('main-shop');
    expect(screen.queryByRole('button', { name: 'Add endpoint' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Rotate URL' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Delete' })).toBeNull();
  });

  it('renders nothing without declared webhooks', () => {
    const { container } = renderSection({ declared: [] });
    expect(container.textContent).toBe('');
    expect(listWebhookEndpoints).not.toHaveBeenCalled();
  });

  it('shows the URL of a created endpoint once', async () => {
    vi.mocked(listWebhookEndpoints).mockResolvedValue([]);
    vi.mocked(createWebhookEndpoint).mockResolvedValue(issued());
    const user = userEvent.setup();
    renderSection();

    await user.click(
      await screen.findByRole('button', { name: 'Add endpoint' }),
    );
    const form = screen.getByRole('dialog', { name: 'Add endpoint' });
    await user.type(within(form).getByLabelText(/Name/), 'main-shop');
    await user.click(dialogButton(form, 'Create'));

    await waitFor(() =>
      expect(createWebhookEndpoint).toHaveBeenCalledWith(extensionId, {
        webhook: 'orders',
        endpoint: 'main-shop',
      }),
    );
    const shown = await screen.findByRole('dialog', {
      name: 'Copy the URL of main-shop now',
    });
    expect(
      within(shown).getByDisplayValue(
        'https://attricat.example/api/public/extension-webhooks/secret-token',
      ),
    ).toBeTruthy();
    expect(within(shown).getByText(/shown only once/)).toBeTruthy();
    expect(dialogButton(shown, 'Copy URL')).toBeTruthy();
    await user.click(dialogButton(shown, 'Close'));
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('explains a path the operator must prefix with the public address', async () => {
    vi.mocked(listWebhookEndpoints).mockResolvedValue([]);
    vi.mocked(createWebhookEndpoint).mockResolvedValue(issued({ url: null }));
    const user = userEvent.setup();
    renderSection();

    await user.click(
      await screen.findByRole('button', { name: 'Add endpoint' }),
    );
    const form = screen.getByRole('dialog', { name: 'Add endpoint' });
    await user.type(within(form).getByLabelText(/Name/), 'main-shop');
    await user.click(dialogButton(form, 'Create'));

    const shown = await screen.findByRole('dialog', {
      name: 'Copy the URL of main-shop now',
    });
    expect(
      within(shown).getByDisplayValue(
        '/api/public/extension-webhooks/secret-token',
      ),
    ).toBeTruthy();
    expect(
      within(shown).getByText(/Prefix the path with the public address/),
    ).toBeTruthy();
    expect(dialogButton(shown, 'Copy path')).toBeTruthy();
  });

  it('checks the endpoint name before sending it', async () => {
    vi.mocked(listWebhookEndpoints).mockResolvedValue([]);
    const user = userEvent.setup();
    renderSection();

    await user.click(
      await screen.findByRole('button', { name: 'Add endpoint' }),
    );
    const form = screen.getByRole('dialog', { name: 'Add endpoint' });
    await user.type(within(form).getByLabelText(/Name/), 'main shop');
    await user.click(dialogButton(form, 'Create'));

    expect(
      within(form).getByText("Use 1 to 128 letters, digits, '.', '_' or '-'."),
    ).toBeTruthy();
    expect(createWebhookEndpoint).not.toHaveBeenCalled();
  });

  it('says when the endpoint name is taken', async () => {
    vi.mocked(listWebhookEndpoints).mockResolvedValue([endpoint]);
    vi.mocked(createWebhookEndpoint).mockRejectedValue(
      new ApiRequestError(409, 'code is already taken', 'code_taken', {
        resource: webhookEndpointResource,
      }),
    );
    const user = userEvent.setup();
    renderSection();

    await user.click(
      await screen.findByRole('button', { name: 'Add endpoint' }),
    );
    const form = screen.getByRole('dialog', { name: 'Add endpoint' });
    await user.type(within(form).getByLabelText(/Name/), 'main-shop');
    await user.click(dialogButton(form, 'Create'));

    expect(
      await within(form).findByText(
        'An endpoint with this name already exists.',
      ),
    ).toBeTruthy();
  });

  it('asks before rotating and shows the new URL', async () => {
    vi.mocked(listWebhookEndpoints).mockResolvedValue([endpoint]);
    vi.mocked(rotateWebhookEndpoint).mockResolvedValue(
      issued({
        url: 'https://attricat.example/api/public/extension-webhooks/new-token',
      }),
    );
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByRole('button', { name: 'Rotate URL' }));
    expect(rotateWebhookEndpoint).not.toHaveBeenCalled();
    const confirm = screen.getByRole('dialog', {
      name: 'Rotate the URL of main-shop?',
    });
    await user.click(dialogButton(confirm, 'Rotate URL'));

    await waitFor(() =>
      expect(rotateWebhookEndpoint).toHaveBeenCalledWith(
        extensionId,
        'main-shop',
      ),
    );
    const shown = await screen.findByRole('dialog', {
      name: 'Copy the URL of main-shop now',
    });
    expect(
      within(shown).getByDisplayValue(
        'https://attricat.example/api/public/extension-webhooks/new-token',
      ),
    ).toBeTruthy();
  });

  it('asks before deleting an endpoint', async () => {
    vi.mocked(listWebhookEndpoints).mockResolvedValue([endpoint]);
    vi.mocked(deleteWebhookEndpoint).mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByRole('button', { name: 'Delete' }));
    expect(deleteWebhookEndpoint).not.toHaveBeenCalled();
    const confirm = screen.getByRole('dialog', { name: 'Delete main-shop?' });
    await user.click(dialogButton(confirm, 'Delete'));

    await waitFor(() =>
      expect(deleteWebhookEndpoint).toHaveBeenCalledWith(
        extensionId,
        'main-shop',
      ),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });
});
