// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import i18n from '../../i18n';
import { currentSession } from '../auth/api';
import { getSystemHealth } from './api';
import { SystemHealthPage } from './SystemHealthPage';

vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./api', () => ({ getSystemHealth: vi.fn() }));

const session = (canRead: boolean) =>
  ({
    workspace_id: 'workspace-one',
    capabilities: { data_health_read: canRead },
  }) as Awaited<ReturnType<typeof currentSession>>;

const renderPage = () => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <SystemHealthPage />
    </QueryClientProvider>,
  );
};

beforeEach(async () => {
  await i18n.changeLanguage('en');
  vi.mocked(currentSession).mockResolvedValue(session(true));
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe('SystemHealthPage', () => {
  it('does not request system health without permission', async () => {
    vi.mocked(currentSession).mockResolvedValue(session(false));
    renderPage();
    expect(
      await screen.findByText(
        'You are not authorized to view system health in this workspace.',
      ),
    ).toBeTruthy();
    expect(getSystemHealth).not.toHaveBeenCalled();
  });

  it('shows the version, branch, and commit the API was built from', async () => {
    vi.mocked(getSystemHealth).mockResolvedValue({
      build: {
        version: '0.1.0',
        branch: 'main',
        commit: '5748be4d1c0ffee5748be4d1c0ffee5748be4d1',
      },
    });
    renderPage();
    expect(await screen.findByText('0.1.0')).toBeTruthy();
    expect(screen.getByText('API version')).toBeTruthy();
    expect(screen.getByText('main')).toBeTruthy();
    expect(
      screen.getByText('5748be4d1c0ffee5748be4d1c0ffee5748be4d1'),
    ).toBeTruthy();
  });

  it('offers a retry when the build details cannot be loaded', async () => {
    vi.mocked(getSystemHealth).mockRejectedValue(new Error('offline'));
    renderPage();
    expect(
      await screen.findByText('Unable to load this information'),
    ).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeTruthy();
  });
});
