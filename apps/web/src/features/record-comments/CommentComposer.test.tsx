// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import { CommentComposer } from './CommentComposer';
import { createComment, updateComment } from './api';
import { getDirectory } from '../principals/api';

vi.mock('./api', () => ({ createComment: vi.fn(), updateComment: vi.fn() }));
vi.mock('../principals/api', () => ({ getDirectory: vi.fn() }));
vi.mock('../auth/api', () => ({
  currentSession: async () => ({
    user_id: '00000000-0000-4000-8000-000000000201',
    workspace_id: '00000000-0000-4000-8000-000000000002',
  }),
}));

const renderComposer = (
  props: Partial<React.ComponentProps<typeof CommentComposer>> = {},
) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <CommentComposer recordId="record" onSaved={vi.fn()} {...props} />
    </QueryClientProvider>,
  );
};

const ada = '6a1f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f60';
const bob = '7b2f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f61';

beforeEach(() => {
  vi.clearAllMocks();
  sessionStorage.clear();
  vi.mocked(getDirectory).mockResolvedValue({
    users: [
      {
        id: ada,
        display_name: 'Ada Lovelace',
        email: 'ada@example.test',
        active: true,
      },
      {
        id: bob,
        display_name: 'Bob',
        email: 'bob@example.test',
        active: true,
      },
    ],
    teams: [],
  });
});

describe('comment composer', () => {
  it('previews Markdown and submits a comment independently of record editing', async () => {
    vi.mocked(createComment).mockResolvedValue(undefined);
    const saved = vi.fn();
    renderComposer({ onSaved: saved });
    const user = userEvent.setup();
    await user.type(
      screen.getByLabelText('Comment', { exact: false }),
      '**Hello**',
    );
    await user.click(screen.getByRole('button', { name: 'Preview Markdown' }));
    expect(screen.getByText('Hello').tagName).toBe('STRONG');
    await user.click(screen.getByRole('button', { name: 'Post comment' }));
    await waitFor(() => expect(saved).toHaveBeenCalledOnce());
    expect(createComment).toHaveBeenCalledWith('record', '**Hello**');
  });

  it('retains input on failure and does not submit when disabled', async () => {
    vi.mocked(createComment).mockRejectedValue(
      new Error('Network unavailable'),
    );
    const { unmount } = renderComposer();
    const user = userEvent.setup();
    await user.type(
      screen.getByLabelText('Comment', { exact: false }),
      'Keep me',
    );
    await user.click(screen.getByRole('button', { name: 'Post comment' }));
    await screen.findByText('Network unavailable');
    expect((screen.getByRole('textbox') as HTMLTextAreaElement).value).toBe(
      'Keep me',
    );
    unmount();
    sessionStorage.clear();
    renderComposer({ disabled: true });
    expect((screen.getByRole('textbox') as HTMLTextAreaElement).disabled).toBe(
      true,
    );
    expect(
      (
        screen.getByRole('button', {
          name: 'Post comment',
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
  });

  it('preserves edits on conflict and offers reload without clearing the draft', async () => {
    vi.mocked(updateComment).mockRejectedValue(
      new ApiRequestError(409, 'Conflict', 'comment_conflict'),
    );
    const reload = vi.fn().mockResolvedValue(undefined);
    renderComposer({
      comment: {
        id: 'comment',
        author_user_id: 'user',
        author_display_name: 'Reader',
        author_email: 'reader@example.com',
        author_kind: 'person',
        body: 'Original',
        revision: 2,
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-01T00:00:00Z',
      },
      onReload: reload,
    });
    const user = userEvent.setup();
    await user.type(screen.getByRole('textbox'), ' edited');
    await user.click(screen.getByRole('button', { name: 'Save changes' }));
    await screen.findByRole('button', {
      name: 'Reload comments and keep draft',
    });
    expect((screen.getByRole('textbox') as HTMLTextAreaElement).value).toBe(
      'Original edited',
    );
    expect(updateComment).toHaveBeenCalledWith(
      'record',
      'comment',
      2,
      'Original edited',
    );
    await user.click(
      screen.getByRole('button', { name: 'Reload comments and keep draft' }),
    );
    expect(reload).toHaveBeenCalledOnce();
  });

  it('suggests workspace members after @ and inserts the chosen mention', async () => {
    Element.prototype.scrollIntoView = vi.fn();
    vi.mocked(createComment).mockResolvedValue(undefined);
    renderComposer();
    const user = userEvent.setup();
    const input = screen.getByRole('textbox') as HTMLTextAreaElement;
    await user.type(input, 'Thanks @lo');
    const option = await screen.findByRole('option', { name: /Ada Lovelace/ });
    expect(option.getAttribute('aria-selected')).toBe('true');
    expect(screen.queryByRole('option', { name: /Bob/ })).toBeNull();
    await user.keyboard('{Enter}');
    expect(input.value).toBe(`Thanks [@Ada Lovelace](user:${ada}) `);
    expect(screen.queryByRole('listbox')).toBeNull();

    await user.type(input, 'and @');
    expect(await screen.findAllByRole('option')).toHaveLength(2);
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('listbox')).toBeNull();
    await user.type(input, 'b');
    await user.click(await screen.findByRole('option', { name: /Bob/ }));
    await user.click(screen.getByRole('button', { name: 'Post comment' }));
    expect(createComment).toHaveBeenCalledWith(
      'record',
      `Thanks [@Ada Lovelace](user:${ada}) and [@Bob](user:${bob}) `,
    );
  });
});
