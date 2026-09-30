// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { createToken, listTokenPermissions } from './api';
import { profileQueryKeys } from './queryKeys';
import { PersonalTokensPage } from './PersonalTokensPage';

vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./api', () => ({
  createToken: vi.fn(),
  listTokenPermissions: vi.fn(),
}));

describe('PersonalTokensPage', () => {
  it('submits only once while token creation is pending', async () => {
    const client = new QueryClient();
    const session = { capabilities: { tokens_manage: true } };
    const permissions = [
      { code: 'entities.read', description: 'Read entities' },
    ];
    vi.mocked(currentSession).mockResolvedValue(session as never);
    vi.mocked(listTokenPermissions).mockResolvedValue(permissions);
    client.setQueryData(authQueryKeys.session(), session);
    client.setQueryData(profileQueryKeys.tokenPermissions(), permissions);
    vi.mocked(createToken).mockImplementation(() => new Promise(() => {}));
    const view = render(
      <QueryClientProvider client={client}>
        <ToastProvider>
          <PersonalTokensPage />
        </ToastProvider>
      </QueryClientProvider>,
    );
    fireEvent.change(screen.getByRole('textbox', { name: 'Label' }), {
      target: { value: 'My token' },
    });
    fireEvent.click(screen.getByRole('checkbox', { name: 'entities.read' }));
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
});
