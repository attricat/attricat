// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import { currentSession } from '../auth/api';
import { getUserProfile } from './api';
import { UserProfilePage } from './UserProfilePage';

vi.mock(
  '@tanstack/react-router',
  async () => (await import('../../test/routerLinkMock')).routerLinkMock,
);
vi.mock('./api', () => ({ getUserProfile: vi.fn() }));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));

const ada = '6a1f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f60';
const viewer = '7b2f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f61';
const profile = {
  id: ada,
  display_name: 'Ada Lovelace',
  email: 'ada@example.test',
  avatar_file_id: null,
  active: true,
  member_since: '2026-01-05T10:00:00Z',
  teams: [
    { id: '8c3f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f62', code: 'qa', name: 'QA' },
  ],
};

const renderPage = (userId = ada) =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <UserProfilePage userId={userId} />
    </QueryClientProvider>,
  );

const session = (userId: string) =>
  ({ user_id: userId }) as Awaited<ReturnType<typeof currentSession>>;

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe('UserProfilePage', () => {
  it('shows a member with their email and teams', async () => {
    vi.mocked(getUserProfile).mockResolvedValue(profile);
    vi.mocked(currentSession).mockResolvedValue(session(viewer));
    renderPage();
    expect(
      await screen.findByRole('heading', { name: 'Ada Lovelace' }),
    ).toBeTruthy();
    expect(
      screen
        .getByRole('link', { name: 'ada@example.test' })
        .getAttribute('href'),
    ).toBe('mailto:ada@example.test');
    expect(screen.getByText('Active member')).toBeTruthy();
    expect(screen.getByText('QA')).toBeTruthy();
    expect(
      screen.queryByRole('link', { name: 'Edit your profile' }),
    ).toBeNull();
  });

  it('explains a former member without teams', async () => {
    vi.mocked(getUserProfile).mockResolvedValue({
      ...profile,
      display_name: null,
      active: false,
      teams: [],
    });
    vi.mocked(currentSession).mockResolvedValue(session(viewer));
    renderPage();
    expect(
      await screen.findByRole('heading', { name: 'ada@example.test' }),
    ).toBeTruthy();
    expect(screen.getByText('No longer an active member')).toBeTruthy();
    expect(screen.getByText('Not a member of any team')).toBeTruthy();
  });

  it('offers your own profile settings', async () => {
    vi.mocked(getUserProfile).mockResolvedValue(profile);
    vi.mocked(currentSession).mockResolvedValue(session(ada));
    renderPage();
    expect(
      (
        await screen.findByRole('link', { name: 'Edit your profile' })
      ).getAttribute('href'),
    ).toBe('/profile');
  });

  it('says when the user is not a workspace member', async () => {
    vi.mocked(getUserProfile).mockRejectedValue(
      new ApiRequestError(404, 'not found'),
    );
    vi.mocked(currentSession).mockResolvedValue(session(viewer));
    renderPage();
    expect(
      await screen.findByText('This user is not a member of the workspace.'),
    ).toBeTruthy();
    expect(
      screen.getByRole('heading', { name: 'User not found' }),
    ).toBeTruthy();
  });
});
