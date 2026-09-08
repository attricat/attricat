// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import '../i18n';
import { AppLayout, SessionErrorState } from './AppLayout';
import { authQueryKeys } from '../features/auth/query-keys';

const { currentSessionMock, logoutMock, navigateMock } = vi.hoisted(() => ({
  currentSessionMock: vi.fn(),
  logoutMock: vi.fn(),
  navigateMock: vi.fn(),
}));

vi.mock('@tanstack/react-router', () => ({
  Navigate: () => null,
  Outlet: () => null,
  useNavigate: () => navigateMock,
  useRouterState: ({ select }: { select: (state: unknown) => unknown }) =>
    select({ location: { pathname: '/catalog' } }),
}));

vi.mock('@mui/material', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@mui/material')>()),
  useMediaQuery: () => true,
}));

vi.mock('../features/auth/api', () => ({
  currentSession: currentSessionMock,
  logout: logoutMock,
}));

vi.mock('./SideNavigation', () => ({
  drawerWidth: 264,
  SideNavigation: ({ onSignOut }: { onSignOut?: () => void }) => (
    <button onClick={onSignOut}>Sign out</button>
  ),
}));

const session = {
  email: 'user@example.test',
  login_identifier: 'example.local',
  user_id: '123e4567-e89b-12d3-a456-426614174000',
  display_name: null,
  workspace_id: '223e4567-e89b-12d3-a456-426614174000',
};

const renderAppLayout = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const clear = vi.spyOn(queryClient, 'clear');
  render(
    <QueryClientProvider client={queryClient}>
      <AppLayout />
    </QueryClientProvider>,
  );
  return { clear, queryClient };
};

describe('SessionErrorState', () => {
  it('renders an accessible session error with a retry control', () => {
    const markup = renderToStaticMarkup(
      <SessionErrorState onRetry={vi.fn()} />,
    );

    expect(markup).toContain('role="alert"');
    expect(markup).toContain('We couldn&#x27;t restore your session');
    expect(markup).toContain('Retry');
    expect(markup).toMatch(/<button[^>]*>Retry<\/button>/);
  });
});

describe('AppLayout sign out', () => {
  it('keeps the session and provides a retry when sign out fails, then clears it after a successful retry', async () => {
    currentSessionMock.mockResolvedValue(session);
    logoutMock.mockRejectedValueOnce(new Error('Service unavailable'));
    logoutMock.mockResolvedValueOnce(undefined);
    const { clear, queryClient } = renderAppLayout();
    const user = userEvent.setup();

    await user.click(await screen.findByRole('button', { name: 'Sign out' }));

    expect((await screen.findByRole('alert')).textContent).toContain(
      "We couldn't sign you out. Check your connection and try again.",
    );
    expect(screen.getByRole('button', { name: 'Retry sign out' })).toBeTruthy();
    expect(clear).not.toHaveBeenCalled();
    expect(queryClient.getQueryData(authQueryKeys.session())).toEqual(session);
    expect(navigateMock).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Retry sign out' }));

    await waitFor(() => expect(clear).toHaveBeenCalledOnce());
    expect(navigateMock).toHaveBeenCalledWith({ to: '/login' });
  });
});
