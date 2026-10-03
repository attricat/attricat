// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import '../i18n';
import { AppLayout } from './AppLayout';
import { SessionErrorState } from './SessionErrorStates';
import { authQueryKeys } from '../features/auth/queryKeys';

let pathname = '/catalog';
let isDesktop = true;

const { currentSessionMock, logoutMock, navigateMock } = vi.hoisted(() => ({
  currentSessionMock: vi.fn(),
  logoutMock: vi.fn(),
  navigateMock: vi.fn(),
}));

vi.mock('@tanstack/react-router', () => ({
  Navigate: ({ to }: { to: string }) => (
    <div data-testid="navigate" data-to={to} />
  ),
  Outlet: () => null,
  useNavigate: () => navigateMock,
  useRouterState: ({ select }: { select: (state: unknown) => unknown }) =>
    select({ location: { pathname } }),
}));

vi.mock('@mui/material', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@mui/material')>()),
  useMediaQuery: () => isDesktop,
}));

vi.mock('../features/auth/api', () => ({
  currentSession: currentSessionMock,
  logout: logoutMock,
}));

// Shell-level extension surfaces have their own tests.
vi.mock('../features/extensions/ExtensionActionDialogHost', () => ({
  ExtensionActionDialogHost: () => null,
}));
vi.mock('../features/extension-runs/ExtensionRunWatcher', () => ({
  ExtensionRunWatcher: () => null,
}));

vi.mock('./SideNavigation', () => ({
  SideNavigation: ({
    onCompactExploreOpenChange,
    onSignOut,
  }: {
    onCompactExploreOpenChange?: (open: boolean) => void;
    onSignOut?: () => void;
  }) => (
    <>
      <button onClick={() => onCompactExploreOpenChange?.(true)}>
        Open Explore
      </button>
      <button onClick={onSignOut}>Sign out</button>
    </>
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
  const view = render(
    <QueryClientProvider client={queryClient}>
      <AppLayout />
    </QueryClientProvider>,
  );
  return { clear, queryClient, unmount: view.unmount };
};

describe('SessionErrorState', () => {
  it('renders an accessible session error with a retry control', () => {
    const markup = renderToStaticMarkup(
      <SessionErrorState onRetry={vi.fn()} onSignOut={vi.fn()} />,
    );

    expect(markup).toContain('role="alert"');
    expect(markup).toContain('We couldn&#x27;t restore your session');
    expect(markup).toContain('Retry');
    expect(markup).toMatch(/<button[^>]*>Retry<\/button>/);
    expect(markup).toMatch(/<button[^>]*>Sign out<\/button>/);
  });
});

describe('AppLayout navigation', () => {
  it('uses the expanded navigation width on mobile', async () => {
    isDesktop = false;
    currentSessionMock.mockResolvedValue(session);
    renderAppLayout();
    const user = userEvent.setup();

    await user.click(
      await screen.findByRole('button', { name: 'Open navigation' }),
    );

    const navigation = await screen.findByRole('button', { name: 'Sign out' });
    const drawerPaper = navigation.closest('.MuiDrawer-paper');
    expect(drawerPaper).not.toBeNull();
    expect(window.getComputedStyle(drawerPaper as Element).width).toBe('264px');
    isDesktop = true;
  });

  it('expands the desktop drawer when the Explore panel opens', async () => {
    currentSessionMock.mockResolvedValue(session);
    renderAppLayout();
    const user = userEvent.setup();
    const navigation = await screen.findByRole('button', { name: 'Sign out' });
    const drawerPaper = navigation.closest('.MuiDrawer-paper');

    await user.click(screen.getByRole('button', { name: 'Open Explore' }));

    await waitFor(() =>
      expect(window.getComputedStyle(drawerPaper as Element).width).toBe(
        '336px',
      ),
    );
  });

  it('toggles the desktop panel from the edge chevron and remembers a collapse', async () => {
    const stored = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => stored.get(key) ?? null,
      removeItem: (key: string) => stored.delete(key),
      setItem: (key: string, value: string) => stored.set(key, value),
    });
    pathname = '/';
    currentSessionMock.mockResolvedValue(session);
    const { unmount } = renderAppLayout();
    const user = userEvent.setup();
    const paperWidth = async () =>
      window.getComputedStyle(
        (await screen.findByRole('button', { name: 'Sign out' })).closest(
          '.MuiDrawer-paper',
        ) as Element,
      ).width;
    expect(await paperWidth()).toBe('336px');

    await user.click(
      screen.getByRole('button', { name: 'Collapse navigation panel' }),
    );
    expect(await paperWidth()).toBe('88px');

    unmount();
    renderAppLayout();
    expect(await paperWidth()).toBe('88px');

    await user.click(
      screen.getByRole('button', { name: 'Expand navigation panel' }),
    );
    expect(await paperWidth()).toBe('336px');
    expect(stored.has('attricat.navigation-panel-collapsed')).toBe(false);

    await user.click(
      screen.getByRole('button', { name: 'Collapse navigation panel' }),
    );
    await user.click(screen.getByRole('button', { name: 'Open Explore' }));
    expect(await paperWidth()).toBe('336px');
    expect(stored.has('attricat.navigation-panel-collapsed')).toBe(false);
    pathname = '/catalog';
    vi.unstubAllGlobals();
  });

  it('hides the edge chevron on routes without a panel', async () => {
    currentSessionMock.mockResolvedValue(session);
    renderAppLayout();
    await screen.findByRole('button', { name: 'Sign out' });

    expect(
      screen.queryByRole('button', { name: /navigation panel/ }),
    ).toBeNull();
  });
});

describe('AppLayout sign out', () => {
  it('redirects signed-in users away from login', async () => {
    pathname = '/login';
    currentSessionMock.mockResolvedValue(session);
    renderAppLayout();

    expect((await screen.findByTestId('navigate')).dataset.to).toBe('/');
    pathname = '/catalog';
  });

  it('redirects to login from a session error even when sign out fails', async () => {
    currentSessionMock.mockRejectedValueOnce(new Error('Service unavailable'));
    logoutMock.mockRejectedValueOnce(new Error('Service unavailable'));
    renderAppLayout();
    const user = userEvent.setup();

    await user.click(await screen.findByRole('button', { name: 'Sign out' }));

    await waitFor(() =>
      expect(navigateMock).toHaveBeenCalledWith({ to: '/login' }),
    );
  });

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
