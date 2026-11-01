// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import { StrictMode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from './api';
import { SingleSignOnCompletePage } from './SingleSignOnCompletePage';
import { rememberReturnLocation } from './returnLocation';

const navigate = vi.hoisted(() => vi.fn());
vi.mock('@tanstack/react-router', () => ({
  createLink:
    (Component: React.ElementType) =>
    ({ to, ...props }: { to: string } & Record<string, unknown>) => (
      <Component href={to} {...props} />
    ),
  useNavigate: () => navigate,
}));
vi.mock('./api', () => ({ currentSession: vi.fn() }));

const renderPage = (error?: string) =>
  render(
    <StrictMode>
      <QueryClientProvider client={new QueryClient()}>
        <SingleSignOnCompletePage error={error} />
      </QueryClientProvider>
    </StrictMode>,
  );

describe('SingleSignOnCompletePage', () => {
  afterEach(() => {
    vi.clearAllMocks();
    window.sessionStorage.clear();
  });

  it('loads the new session and returns where the person was', async () => {
    window.history.pushState({}, '', '/manage/data-health?x=1');
    rememberReturnLocation();
    window.history.pushState({}, '', '/oidc/complete');
    vi.mocked(currentSession).mockResolvedValue(null);
    renderPage();
    await waitFor(() =>
      expect(navigate).toHaveBeenCalledWith({
        replace: true,
        to: '/manage/data-health?x=1',
      }),
    );
    expect(currentSession).toHaveBeenCalled();
  });

  it('explains a failure by its code, and offers a way back', () => {
    renderPage('oidc_identity_not_linked');
    expect(screen.getByRole('alert').textContent).toContain(
      'no access to this workspace',
    );
    expect(screen.getByRole('link', { name: 'Back to sign-in' })).toBeTruthy();
    expect(navigate).not.toHaveBeenCalled();
    expect(currentSession).not.toHaveBeenCalled();
  });

  it('does not repeat an unknown code to the reader', () => {
    renderPage('<b>made-up</b>');
    expect(screen.getByRole('alert').textContent).toBe(
      'The sign-in could not be completed. Try again.',
    );
  });
});
