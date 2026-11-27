// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { createToken, listTokenPermissions } from './api';
import { profileQueryKeys } from './queryKeys';
import { CreatePersonalTokenPage } from './CreatePersonalTokenPage';

vi.mock('@tanstack/react-router', () => ({
  createLink:
    (Component: React.ElementType) =>
    ({ to, ...props }: { to: string } & Record<string, unknown>) => (
      <Component href={to} {...props} />
    ),
  Link: ({ children }: { children: ReactNode }) => <a href="/">{children}</a>,
  useNavigate: () => vi.fn(),
}));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./api', () => ({
  createToken: vi.fn(),
  listTokenPermissions: vi.fn(),
}));

describe('CreatePersonalTokenPage', () => {
  it('submits only once while token creation is pending', async () => {
    const client = new QueryClient();
    const session = { capabilities: { tokens_manage: true } };
    const permissions = [{ code: 'records.read', description: 'Read records' }];
    vi.mocked(currentSession).mockResolvedValue(session as never);
    vi.mocked(listTokenPermissions).mockResolvedValue(permissions);
    client.setQueryData(authQueryKeys.session(), session);
    client.setQueryData(profileQueryKeys.tokenPermissions(), permissions);
    vi.mocked(createToken).mockImplementation(() => new Promise(() => {}));
    const view = render(
      <QueryClientProvider client={client}>
        <ToastProvider>
          <CreatePersonalTokenPage />
        </ToastProvider>
      </QueryClientProvider>,
    );
    fireEvent.change(screen.getByRole('textbox', { name: 'Label' }), {
      target: { value: 'My token' },
    });
    fireEvent.click(screen.getByRole('checkbox', { name: 'records.read' }));
    const form = view.container.querySelector('form')!;
    fireEvent.submit(form);
    fireEvent.submit(form);
    await waitFor(() => expect(createToken).toHaveBeenCalledOnce());
    expect(
      screen
        .getByRole('button', { name: 'Create token' })
        .hasAttribute('disabled'),
    ).toBe(true);
  });

  it('gives support access no way to outlast the token, and a week by default', async () => {
    const client = new QueryClient();
    const session = { capabilities: { tokens_manage: true } };
    const permissions = [
      'records.read',
      'audit.read',
      'tokens.manage',
      'members.manage',
      'roles.grant',
      'extensions.manage',
    ].map((code) => ({ code, description: code }));
    vi.mocked(currentSession).mockResolvedValue(session as never);
    vi.mocked(listTokenPermissions).mockResolvedValue(permissions);
    client.setQueryData(authQueryKeys.session(), session);
    client.setQueryData(profileQueryKeys.tokenPermissions(), permissions);
    vi.mocked(createToken).mockImplementation(() => new Promise(() => {}));
    const view = render(
      <QueryClientProvider client={client}>
        <ToastProvider>
          <CreatePersonalTokenPage />
        </ToastProvider>
      </QueryClientProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: /Support access/ }));
    const checked = (code: string) =>
      (screen.getByRole('checkbox', { name: code }) as HTMLInputElement)
        .checked;
    expect(checked('records.read')).toBe(true);
    expect(checked('audit.read')).toBe(true);
    for (const code of [
      'tokens.manage',
      'members.manage',
      'roles.grant',
      'extensions.manage',
    ])
      expect(checked(code)).toBe(false);
    const expiry = view.container.querySelector<HTMLInputElement>(
      'input[type="datetime-local"]',
    )!;
    expect(expiry.value).not.toBe('');
    fireEvent.change(screen.getByRole('textbox', { name: 'Label' }), {
      target: { value: 'Support' },
    });
    fireEvent.submit(view.container.querySelector('form')!);
    await waitFor(() => expect(createToken).toHaveBeenCalled());
    const input = vi.mocked(createToken).mock.calls.at(-1)![0];
    expect(input.permissions.sort()).toEqual(['audit.read', 'records.read']);
    const days =
      (new Date(input.expires_at!).getTime() - Date.now()) /
      (24 * 60 * 60 * 1000);
    expect(days).toBeGreaterThan(6.9);
    expect(days).toBeLessThanOrEqual(7);
  });
});
