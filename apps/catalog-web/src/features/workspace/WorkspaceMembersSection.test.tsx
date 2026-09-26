// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { grantMemberRole, listAssignableRoles, listMembers } from './api';
import { WorkspaceMembersSection } from './WorkspaceMembersSection';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  grantMemberRole: vi.fn(),
  listAssignableRoles: vi.fn(),
  listMembers: vi.fn(),
}));

const workspaceId = '123e4567-e89b-12d3-a456-426614174000';

describe('WorkspaceMembersSection', () => {
  it('hides grant controls without roles.grant', async () => {
    vi.mocked(listMembers).mockResolvedValue([
      {
        id: '123e4567-e89b-12d3-a456-426614174001',
        email: 'member@example.test',
        state: 'active',
        grants: [{ id: 'grant-id', role_code: 'reader' }],
      },
    ] as never);
    vi.mocked(listAssignableRoles).mockResolvedValue([]);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <WorkspaceMembersSection canManage workspaceId={workspaceId} />
      </QueryClientProvider>,
    );
    await screen.findByRole('listitem');
    expect(screen.queryByRole('button', { name: /Revoke/ })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Grant role' })).toBeNull();
  });
  it('does not grant the same role twice during a pending request', async () => {
    vi.mocked(listMembers).mockResolvedValue([
      {
        id: '123e4567-e89b-12d3-a456-426614174001',
        email: 'member@example.test',
        state: 'active',
        grants: [],
      },
    ] as never);
    vi.mocked(listAssignableRoles).mockResolvedValue([
      {
        id: '123e4567-e89b-12d3-a456-426614174002',
        code: 'reader',
      },
    ] as never);
    vi.mocked(grantMemberRole).mockImplementation(() => new Promise(() => {}));
    const view = render(
      <QueryClientProvider client={new QueryClient()}>
        <WorkspaceMembersSection
          canManage
          canGrantRoles
          workspaceId={workspaceId}
        />
      </QueryClientProvider>,
    );
    fireEvent.mouseDown(
      await screen.findByRole('combobox', { name: 'Member' }),
    );
    fireEvent.click(
      await screen.findByRole('option', { name: 'member@example.test' }),
    );
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Role' }));
    fireEvent.click(await screen.findByRole('option', { name: 'reader' }));
    const form = view.container.querySelector('form')!;
    fireEvent.submit(form);
    fireEvent.submit(form);
    await waitFor(() => expect(grantMemberRole).toHaveBeenCalledOnce());
    expect(
      screen
        .getByRole('button', { name: 'Grant role' })
        .hasAttribute('disabled'),
    ).toBe(true);
  });
});
