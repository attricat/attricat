// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { discoverWorkspace, fetchSampleLogins, login } from './api';
import { PasswordLoginPage, WorkspaceLoginPage } from './LoginPage';
import { authQueryKeys } from './queryKeys';

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  Navigate: ({ params }: { params: { identifier: string } }) => (
    <span>{`navigate:${params.identifier}`}</span>
  ),
  useNavigate: () => vi.fn(),
}));
vi.mock('../../components/LanguageSwitcher', () => ({
  LanguageSwitcher: () => null,
}));
vi.mock('./api', () => ({
  discoverWorkspace: vi.fn(),
  fetchSampleLogins: vi.fn(),
  login: vi.fn(),
}));

const sampleLogins = (demo: boolean) => ({
  demo,
  login_identifier: 'demo.attricat.com',
  password: 'shared-password',
  accounts: [
    { role: 'viewer', email: 'viewer@attricat.com' },
    { role: 'editor', email: 'editor@attricat.com' },
    { role: 'admin', email: 'admin@attricat.com' },
    { role: 'owner', email: 'owner@attricat.com' },
  ],
});

const renderWithClient = (ui: React.ReactNode, client = new QueryClient()) =>
  render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);

const submitTwice = async (name: string) => {
  const form = (await screen.findByRole('button', { name })).closest('form')!;
  fireEvent.submit(form);
  fireEvent.submit(form);
};

describe('login forms', () => {
  beforeEach(() => vi.mocked(fetchSampleLogins).mockResolvedValue(null));
  afterEach(() => vi.clearAllMocks());

  it('replaces the previous identity cache after login', async () => {
    const session = {
      user_id: 'user-b',
      workspace_id: 'workspace-b',
    } as Awaited<ReturnType<typeof login>>;
    vi.mocked(login).mockResolvedValue(session);
    const client = new QueryClient();
    client.setQueryData(['protected'], 'previous account');
    render(
      <QueryClientProvider client={client}>
        <PasswordLoginPage identifier="default.local" />
      </QueryClientProvider>,
    );
    fireEvent.submit(
      (await screen.findByRole('button', { name: 'Sign in' })).closest('form')!,
    );
    await waitFor(() =>
      expect(client.getQueryData(authQueryKeys.session())).toEqual(session),
    );
    expect(client.getQueryData(['protected'])).toBeUndefined();
  });

  it('submits only one workspace discovery and locks its input while pending', async () => {
    vi.mocked(discoverWorkspace).mockImplementation(
      () => new Promise(() => {}),
    );
    renderWithClient(<WorkspaceLoginPage />);
    await submitTwice('Continue');
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
    await submitTwice('Sign in');
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

  it('sends demo deployments straight to the demo workspace', async () => {
    vi.mocked(fetchSampleLogins).mockResolvedValue(sampleLogins(true));
    renderWithClient(<WorkspaceLoginPage />);
    expect(await screen.findByText('navigate:demo.attricat.com')).toBeTruthy();
  });

  it('signs demo visitors in as the editor and switches accounts', async () => {
    vi.mocked(fetchSampleLogins).mockResolvedValue(sampleLogins(true));
    renderWithClient(<PasswordLoginPage identifier="demo.attricat.com" />);
    const email = (await screen.findByRole('textbox', {
      name: 'Email',
    })) as HTMLInputElement;
    const password = screen.getByLabelText('Password') as HTMLInputElement;
    expect(email.value).toBe('editor@attricat.com');
    expect(password.value).toBe('shared-password');
    expect(screen.queryByText('Change workspace')).toBeNull();
    expect(screen.queryByText('Forgot password?')).toBeNull();

    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Sign in as' }));
    fireEvent.click(
      await screen.findByRole('option', { name: 'Owner · owner@attricat.com' }),
    );
    expect(email.value).toBe('owner@attricat.com');
    expect(password.value).toBe('shared-password');
  });

  it('starts development as the owner and clears the picker for other emails', async () => {
    vi.mocked(fetchSampleLogins).mockResolvedValue(sampleLogins(false));
    renderWithClient(<PasswordLoginPage identifier="demo.attricat.com" />);
    const email = (await screen.findByRole('textbox', {
      name: 'Email',
    })) as HTMLInputElement;
    const picker = screen.getByRole('combobox', { name: 'Sign in as' });
    expect(email.value).toBe('owner@attricat.com');
    expect(picker.textContent).toContain('owner@attricat.com');
    expect(screen.getByText('Change workspace')).toBeTruthy();
    expect(screen.getByText('Forgot password?')).toBeTruthy();

    fireEvent.change(email, { target: { value: 'someone@example.test' } });
    expect(picker.textContent).not.toContain('@');
    fireEvent.change(email, { target: { value: 'Admin@attricat.com' } });
    expect(picker.textContent).toContain('admin@attricat.com');
  });

  it('offers sample accounts outside demos without hiding workspace tools', async () => {
    vi.mocked(fetchSampleLogins).mockResolvedValue(sampleLogins(false));
    renderWithClient(<WorkspaceLoginPage />);
    expect(
      (
        (await screen.findByRole('textbox', {
          name: 'Workspace',
        })) as HTMLInputElement
      ).value,
    ).toBe('demo.attricat.com');
  });
});
