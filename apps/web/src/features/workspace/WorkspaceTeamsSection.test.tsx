// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import {
  createTeam,
  deleteTeam,
  getDirectory,
  listTeams,
  updateTeam,
} from '../principals/api';
import { principalQueryKeys } from '../principals/queryKeys';
import type { Team } from '../principals/schemas';
import { WorkspaceTeamsSection } from './WorkspaceTeamsSection';

vi.mock('../principals/api', () => ({
  createTeam: vi.fn(),
  deleteTeam: vi.fn(),
  getDirectory: vi.fn(),
  listTeams: vi.fn(),
  updateTeam: vi.fn(),
}));

const ada = '6a1f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f60';
const quality: Team = {
  id: '8c3f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f62',
  code: 'qa',
  name: 'Quality',
  member_user_ids: [ada],
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
};

const renderSection = () => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const invalidate = vi.spyOn(client, 'invalidateQueries');
  render(
    <QueryClientProvider client={client}>
      <WorkspaceTeamsSection canManage />
    </QueryClientProvider>,
  );
  return { invalidate };
};

beforeEach(() => {
  vi.mocked(listTeams).mockResolvedValue([quality]);
  vi.mocked(getDirectory).mockResolvedValue({
    users: [
      { id: ada, display_name: 'Ada', email: 'ada@example.test', active: true },
    ],
    teams: [{ id: quality.id, code: 'qa', name: 'Quality', deleted: false }],
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('WorkspaceTeamsSection', () => {
  it('shows progress while teams load', () => {
    vi.mocked(listTeams).mockImplementation(() => new Promise(() => {}));
    renderSection();
    expect(screen.getByRole('progressbar', { name: 'Loading' })).toBeTruthy();
  });

  it('reports a directory failure', async () => {
    vi.mocked(getDirectory).mockRejectedValue(new Error('Directory down'));
    renderSection();
    expect(await screen.findByText('Directory down')).toBeTruthy();
  });

  it('creates a team and refreshes the principals', async () => {
    vi.mocked(createTeam).mockResolvedValue({
      ...quality,
      id: '9d4f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f63',
      code: 'ops',
      name: 'Operations',
      member_user_ids: [],
    });
    const { invalidate } = renderSection();
    await screen.findByText('Quality · qa');
    fireEvent.click(screen.getByRole('button', { name: 'Create team' }));
    const dialog = await screen.findByRole('dialog');
    fireEvent.change(within(dialog).getByRole('textbox', { name: /name/i }), {
      target: { value: 'Operations' },
    });
    fireEvent.change(within(dialog).getByRole('textbox', { name: /code/i }), {
      target: { value: 'ops' },
    });
    fireEvent.submit(dialog.querySelector('form')!);
    await waitFor(() =>
      expect(createTeam).toHaveBeenCalledWith({
        code: 'ops',
        name: 'Operations',
        member_user_ids: [],
      }),
    );
    await waitFor(() =>
      expect(invalidate).toHaveBeenCalledWith({
        queryKey: principalQueryKeys.all(),
      }),
    );
  });

  it('edits a team without changing its code', async () => {
    vi.mocked(updateTeam).mockResolvedValue({ ...quality, name: 'QA' });
    const { invalidate } = renderSection();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Edit Quality' }),
    );
    const dialog = await screen.findByRole('dialog');
    fireEvent.change(within(dialog).getByRole('textbox', { name: /name/i }), {
      target: { value: 'QA' },
    });
    fireEvent.submit(dialog.querySelector('form')!);
    await waitFor(() =>
      expect(updateTeam).toHaveBeenCalledWith(quality.id, {
        name: 'QA',
        member_user_ids: [ada],
      }),
    );
    await waitFor(() =>
      expect(invalidate).toHaveBeenCalledWith({
        queryKey: principalQueryKeys.all(),
      }),
    );
  });

  it('deletes a team and keeps its name while the dialog closes', async () => {
    vi.mocked(deleteTeam).mockResolvedValue(undefined);
    const { invalidate } = renderSection();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Delete team Quality' }),
    );
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText(/Quality/)).toBeTruthy();
    fireEvent.click(
      within(dialog).getByRole('button', { name: 'Delete team' }),
    );
    await waitFor(() => expect(deleteTeam).toHaveBeenCalledWith(quality.id));
    await waitFor(() =>
      expect(invalidate).toHaveBeenCalledWith({
        queryKey: principalQueryKeys.all(),
      }),
    );
    // The confirmation still names the team during the exit transition.
    expect(within(dialog).getByText(/Quality/)).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('clears a previous delete failure when the dialog reopens', async () => {
    vi.mocked(deleteTeam).mockRejectedValueOnce(new Error('Team in use'));
    renderSection();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Delete team Quality' }),
    );
    const dialog = await screen.findByRole('dialog');
    fireEvent.click(
      within(dialog).getByRole('button', { name: 'Delete team' }),
    );
    expect(await within(dialog).findByText('Team in use')).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    fireEvent.click(
      screen.getByRole('button', { name: 'Delete team Quality' }),
    );
    await screen.findByRole('dialog');
    await waitFor(() => expect(screen.queryByText('Team in use')).toBeNull());
  });

  it('keeps the dialog open while a delete runs so its error is seen', async () => {
    let reject: (error: Error) => void = () => {};
    vi.mocked(deleteTeam).mockImplementation(
      () =>
        new Promise((_, rejectDelete) => {
          reject = rejectDelete;
        }),
    );
    renderSection();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Delete team Quality' }),
    );
    const dialog = await screen.findByRole('dialog');
    fireEvent.click(
      within(dialog).getByRole('button', { name: 'Delete team' }),
    );
    const cancel = within(dialog).getByRole('button', { name: 'Cancel' });
    await waitFor(() => expect(cancel.hasAttribute('disabled')).toBe(true));
    fireEvent.keyDown(dialog, { key: 'Escape' });
    reject(new Error('Team in use'));
    expect(await within(dialog).findByText('Team in use')).toBeTruthy();
    expect(screen.getByRole('dialog')).toBe(dialog);
  });
});
