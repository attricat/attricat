// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import { confirmPasswordReset, requestPasswordReset } from './api';
import {
  PasswordResetConfirmationPage,
  PasswordResetRequestPage,
} from './PasswordResetPages';

vi.mock('@tanstack/react-router', () => ({
  createLink:
    (Component: React.ElementType) =>
    ({ to, ...props }: { to: string } & Record<string, unknown>) => (
      <Component href={to} {...props} />
    ),
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  useNavigate: () => vi.fn(),
}));
vi.mock('../../components/LanguageSwitcher', () => ({
  LanguageSwitcher: () => null,
}));
vi.mock('./api', () => ({
  confirmPasswordReset: vi.fn(),
  requestPasswordReset: vi.fn(),
}));

describe('PasswordResetRequestPage', () => {
  it('shows a failed request by its code, then confirms a sent link', async () => {
    vi.mocked(requestPasswordReset)
      .mockRejectedValueOnce(
        new ApiRequestError(429, 'Too many requests', 'rate_limited'),
      )
      .mockResolvedValueOnce(undefined);
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={new QueryClient()}>
        <PasswordResetRequestPage />
      </QueryClientProvider>,
    );

    await user.type(
      screen.getByRole('textbox', { name: 'Email' }),
      'ada@example.com',
    );
    await user.click(screen.getByRole('button', { name: 'Send reset link' }));
    expect(
      await screen.findByText('Too many attempts. Try again later.'),
    ).toBeTruthy();

    await user.click(screen.getByRole('button', { name: 'Send reset link' }));
    expect(await screen.findByText(/a reset link has been sent/)).toBeTruthy();
    expect(requestPasswordReset).toHaveBeenLastCalledWith('ada@example.com');
  });
});

describe('PasswordResetConfirmationPage', () => {
  const renderPage = (token?: string) =>
    render(
      <QueryClientProvider client={new QueryClient()}>
        <PasswordResetConfirmationPage token={token} />
      </QueryClientProvider>,
    );

  it('offers a new link when the link has no token', () => {
    renderPage();
    expect(
      screen.getByText('This password reset link is invalid or expired'),
    ).toBeTruthy();
    expect(
      screen.getByRole('link', { name: 'Request a new link' }),
    ).toBeTruthy();
    expect(screen.queryByLabelText(/New password/)).toBeNull();
  });

  it('resets only once both passwords match', async () => {
    vi.mocked(confirmPasswordReset).mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderPage('token-1');
    await user.type(screen.getByLabelText(/^New password/), 'secret-1!');
    await user.type(screen.getByLabelText(/^Confirm new password/), 'other');
    await user.click(
      screen.getByRole('button', { name: 'Reset your password' }),
    );
    expect(await screen.findByText('Passwords do not match.')).toBeTruthy();
    expect(confirmPasswordReset).not.toHaveBeenCalled();

    await user.clear(screen.getByLabelText(/^Confirm new password/));
    await user.type(
      screen.getByLabelText(/^Confirm new password/),
      'secret-1!',
    );
    await user.click(
      screen.getByRole('button', { name: 'Reset your password' }),
    );
    expect(
      await screen.findByText(/Your password has been reset/),
    ).toBeTruthy();
    expect(confirmPasswordReset).toHaveBeenCalledWith('token-1', 'secret-1!');
  });
});
