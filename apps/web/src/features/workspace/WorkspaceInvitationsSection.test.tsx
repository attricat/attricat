// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import {
  createInvitation,
  createWorkspaceUser,
  listAssignableRoles,
  listInvitations,
} from './api';
import { WorkspaceInvitationsSection } from './WorkspaceInvitationsSection';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  createInvitation: vi.fn(),
  createWorkspaceUser: vi.fn(),
  listAssignableRoles: vi.fn(),
  listInvitations: vi.fn(),
}));
const workspaceId = '123e4567-e89b-12d3-a456-426614174000';
const prepare = () => {
  vi.mocked(listInvitations).mockResolvedValue([]);
  vi.mocked(listAssignableRoles).mockResolvedValue([
    {
      id: '123e4567-e89b-12d3-a456-426614174002',
      code: 'reader',
    },
  ] as never);
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <WorkspaceInvitationsSection canManage workspaceId={workspaceId} />
    </QueryClientProvider>,
  );
};

const submitForm = async (buttonName: string) => {
  const form = screen
    .getByRole('button', { name: buttonName })
    .closest('form')!;
  fireEvent.change(within(form).getByRole('textbox', { name: 'Email' }), {
    target: { value: 'user@example.test' },
  });
  fireEvent.mouseDown(within(form).getByRole('combobox', { name: 'Role' }));
  fireEvent.click(await screen.findByRole('option', { name: 'reader' }));
  fireEvent.change(within(form).getByLabelText(/Expires at/), {
    target: { value: '2099-01-01T12:00' },
  });
  fireEvent.submit(form);
  fireEvent.submit(form);
};

describe('WorkspaceInvitationsSection', () => {
  it('prevents duplicate existing-user invitations', async () => {
    vi.mocked(createInvitation).mockImplementation(() => new Promise(() => {}));
    prepare();
    await submitForm('Create invitation');
    await waitFor(() => expect(createInvitation).toHaveBeenCalledOnce());
    expect(
      screen
        .getByRole('button', { name: 'Create invitation' })
        .hasAttribute('disabled'),
    ).toBe(true);
  });
  it('prevents duplicate workspace-user invitations', async () => {
    vi.mocked(createWorkspaceUser).mockImplementation(
      () => new Promise(() => {}),
    );
    prepare();
    await submitForm('Create user and invite');
    await waitFor(() => expect(createWorkspaceUser).toHaveBeenCalledOnce());
    expect(
      screen
        .getByRole('button', { name: 'Create user and invite' })
        .hasAttribute('disabled'),
    ).toBe(true);
  });
});
