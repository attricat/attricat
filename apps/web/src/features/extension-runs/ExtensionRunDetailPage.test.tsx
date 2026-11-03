// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import { cancelExtensionRun, getExtensionRun } from './api';
import { ExtensionRunDetailPage } from './ExtensionRunDetailPage';

vi.mock('../../components/RouterLink', () => ({
  RouterButton: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
}));
vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  cancelExtensionRun: vi.fn(),
  getExtensionRun: vi.fn(),
}));

const runId = '55555555-5555-4555-8555-555555555555';
const renderPage = (id = runId) =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ExtensionRunDetailPage runId={id} />
    </QueryClientProvider>,
  );

beforeEach(() => {
  vi.resetAllMocks();
});

it('lets the initiator stop a run they can no longer read', async () => {
  vi.mocked(getExtensionRun).mockRejectedValue(
    new ApiRequestError(403, 'Forbidden', 'forbidden'),
  );
  vi.mocked(cancelExtensionRun).mockResolvedValue(undefined);
  renderPage();

  await userEvent.click(
    await screen.findByRole('button', { name: 'Cancel run' }),
  );

  await waitFor(() => expect(cancelExtensionRun).toHaveBeenCalledWith(runId));
  expect(await screen.findByText('Cancelling this run.')).toBeTruthy();
});

it('treats a malformed run ID as not found without asking the server', async () => {
  renderPage('foo');
  await screen.findByRole('alert');
  expect(getExtensionRun).not.toHaveBeenCalled();
});
