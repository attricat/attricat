// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import { getInvitationOptions } from '../auth/api';
import { completeOnboarding } from './api';
import { PasswordSetupPage } from './PasswordSetupPage';

vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('../../components/RouterLink', () => ({
  RouterButton: ({
    children,
    search,
  }: {
    children: React.ReactNode;
    search: { secret: string };
  }) => <span data-secret={search.secret}>{children}</span>,
}));
vi.mock('../auth/api', () => ({ getInvitationOptions: vi.fn() }));
vi.mock('./api', () => ({ completeOnboarding: vi.fn() }));

beforeEach(() => vi.resetAllMocks());

it('leads to accepting when another link already set the password', async () => {
  vi.mocked(getInvitationOptions).mockResolvedValue({
    login_identifier: 'second.example',
    display_name: null,
    password_allowed: true,
  });
  vi.mocked(completeOnboarding).mockRejectedValue(
    new ApiRequestError(422, 'Invalid', 'invitation_invalid'),
  );
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <PasswordSetupPage
        initialInvitationSecret="second-invitation"
        initialOnboardingSecret="used"
      />
    </QueryClientProvider>,
  );
  await vi.waitFor(() => expect(getInvitationOptions).toHaveBeenCalled());
  await screen.findByLabelText('Confirm password');
  for (const input of document.querySelectorAll(
    'input[autocomplete="new-password"]',
  ))
    fireEvent.change(input, { target: { value: 'a long password' } });
  fireEvent.click(
    screen.getByRole('button', { name: 'Set password and join workspace' }),
  );

  const accept = await screen.findByText('Accept with your existing account');
  expect(accept.getAttribute('data-secret')).toBe('second-invitation');
});
