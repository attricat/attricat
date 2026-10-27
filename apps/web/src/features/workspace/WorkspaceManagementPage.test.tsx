// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import { WorkspaceManagementPage } from './WorkspaceManagementPage';

const navigate = vi.hoisted(() => vi.fn());
vi.mock('@tanstack/react-router', () => ({
  Navigate: ({ to }: { to: string }) => {
    navigate(to);
    return null;
  },
}));
vi.mock('../../components/RouterLink', () => ({
  RouterButton: ({ children, to }: { children: ReactNode; to: string }) => (
    <a href={to}>{children}</a>
  ),
}));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./WorkspaceAgentSection', () => ({
  WorkspaceAgentSection: ({ canManage }: { canManage: boolean }) =>
    `agent section ${canManage ? 'managed' : 'refused'}`,
}));
vi.mock('./WorkspaceMembersSection', () => ({
  WorkspaceMembersSection: () => 'members section',
}));

const session = (capabilities: Record<string, boolean>) =>
  ({
    user_id: 'user',
    workspace_id: 'workspace',
    login_identifier: 'acme',
    capabilities,
  }) as never;

const renderPage = (section?: 'agent') =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <WorkspaceManagementPage section={section} />
    </QueryClientProvider>,
  );

describe('WorkspaceManagementPage', () => {
  it('waits for the session before rendering a section', async () => {
    let resolve!: (value: never) => void;
    vi.mocked(currentSession).mockReturnValue(
      new Promise((done) => {
        resolve = done;
      }),
    );
    renderPage('agent');
    expect(screen.queryByText(/agent section/)).toBeNull();
    resolve(session({ agents_manage: true }));
    expect(await screen.findByText('agent section managed')).toBeTruthy();
  });

  it('offers only the sections the caller may manage', async () => {
    vi.mocked(currentSession).mockResolvedValue(
      session({ agents_manage: true }),
    );
    renderPage('agent');
    expect(
      (await screen.findByRole('link', { name: 'Agent' })).getAttribute('href'),
    ).toBe('/manage/workspace/agent');
    expect(screen.queryByRole('link', { name: 'Members' })).toBeNull();
  });

  it('opens the first section the caller may manage', async () => {
    vi.mocked(currentSession).mockResolvedValue(
      session({ agents_manage: true }),
    );
    renderPage();
    await vi.waitFor(() =>
      expect(navigate).toHaveBeenCalledWith('/manage/workspace/agent'),
    );
  });
});
