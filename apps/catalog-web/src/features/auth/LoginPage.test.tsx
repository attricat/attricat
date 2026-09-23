// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { discoverWorkspace, login } from './api';
import { PasswordLoginPage, WorkspaceLoginPage } from './LoginPage';

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  useNavigate: () => vi.fn(),
}));
vi.mock('../../components/LanguageSwitcher', () => ({
  LanguageSwitcher: () => null,
}));
vi.mock('./api', () => ({ discoverWorkspace: vi.fn(), login: vi.fn() }));

const submitTwice = (name: string) => {
  const form = screen.getByRole('button', { name }).closest('form')!;
  fireEvent.submit(form);
  fireEvent.submit(form);
};

describe('login forms', () => {
  afterEach(() => vi.clearAllMocks());

  it('submits only one workspace discovery and locks its input while pending', async () => {
    vi.mocked(discoverWorkspace).mockImplementation(
      () => new Promise(() => {}),
    );
    render(<WorkspaceLoginPage />);
    submitTwice('Continue');
    await waitFor(() => expect(discoverWorkspace).toHaveBeenCalledTimes(1));
    expect(
      (screen.getByRole('textbox', { name: 'Workspace' }) as HTMLInputElement)
        .disabled,
    ).toBe(true);
    expect(
      (screen.getByRole('button', { name: 'Continue' }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it('submits only one password login to avoid rotating the session twice', async () => {
    vi.mocked(login).mockImplementation(() => new Promise(() => {}));
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={client}>
        <PasswordLoginPage identifier="default.local" />
      </QueryClientProvider>,
    );
    submitTwice('Sign in');
    await waitFor(() => expect(login).toHaveBeenCalledTimes(1));
    expect(
      (screen.getByRole('textbox', { name: 'Email' }) as HTMLInputElement)
        .disabled,
    ).toBe(true);
    expect(
      (screen.getByLabelText('Password') as HTMLInputElement).disabled,
    ).toBe(true);
    expect(
      (screen.getByRole('button', { name: 'Sign in' }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });
});
