// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { getDirectory } from './api';
import { PrincipalValue } from './PrincipalValue';

vi.mock('./api', () => ({ getDirectory: vi.fn() }));

const ada = '6a1f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f60';
const former = '7b2f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f61';
const team = '8c3f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f62';
const missing = '9d4f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f63';

const directory = {
  users: [
    { id: ada, display_name: 'Ada', email: 'ada@example.test', active: true },
    {
      id: former,
      display_name: 'Former',
      email: 'former@example.test',
      active: false,
    },
  ],
  teams: [{ id: team, code: 'old', name: 'Old team', deleted: true }],
};

const renderValue = (value: unknown) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <PrincipalValue value={value} />
    </QueryClientProvider>,
  );
};

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe('PrincipalValue', () => {
  it('shows an active user without a caption', async () => {
    vi.mocked(getDirectory).mockResolvedValue(directory);
    renderValue(`user:${ada}`);
    expect(await screen.findByText('Ada')).toBeTruthy();
    expect(screen.queryByText('No longer an active member')).toBeNull();
  });

  it('explains an inactive user', async () => {
    vi.mocked(getDirectory).mockResolvedValue(directory);
    renderValue(`user:${former}`);
    expect(await screen.findByText('Former')).toBeTruthy();
    expect(screen.getByText('No longer an active member')).toBeTruthy();
  });

  it('explains a deleted team', async () => {
    vi.mocked(getDirectory).mockResolvedValue(directory);
    renderValue(`team:${team}`);
    expect(await screen.findByText('Old team')).toBeTruthy();
    expect(screen.getByText('Deleted team')).toBeTruthy();
  });

  it('shows a reference the loaded directory does not list as unknown', async () => {
    vi.mocked(getDirectory).mockResolvedValue(directory);
    renderValue(`user:${missing}`);
    expect(
      await screen.findByText(`Unknown user or team (user:${missing})`),
    ).toBeTruthy();
  });

  it('shows progress while the directory loads', () => {
    vi.mocked(getDirectory).mockImplementation(() => new Promise(() => {}));
    renderValue(`user:${ada}`);
    expect(screen.getByText('Loading…')).toBeTruthy();
  });

  it('reports a failed directory load instead of an unknown reference', async () => {
    vi.mocked(getDirectory).mockRejectedValue(new Error('Directory down'));
    renderValue(`user:${ada}`);
    expect(
      await screen.findByText('Users and teams could not be loaded'),
    ).toBeTruthy();
    expect(screen.queryByText(/Unknown user or team/)).toBeNull();
  });
});
