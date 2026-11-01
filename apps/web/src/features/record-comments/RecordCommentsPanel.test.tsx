// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { RecordCommentsPanel } from './RecordCommentsPanel';
import { deleteComment, listComments } from './api';

const userId = '00000000-0000-4000-8000-000000000201';

vi.mock(
  '@tanstack/react-router',
  async () => (await import('../../test/routerLinkMock')).routerLinkMock,
);
vi.mock('./api', () => ({
  listComments: vi.fn(),
  createComment: vi.fn(),
  updateComment: vi.fn(),
  deleteComment: vi.fn(),
}));
vi.mock('../auth/api', () => ({
  currentSession: async () => ({
    user_id: userId,
    workspace_id: '00000000-0000-4000-8000-000000000002',
  }),
}));

const comment = {
  revision: 1,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
};

beforeEach(() => {
  vi.clearAllMocks();
  sessionStorage.clear();
});

describe('record comments', () => {
  it('shows system comments with a system badge and without an edit action', async () => {
    vi.mocked(listComments).mockResolvedValue({
      has_more: false,
      items: [
        {
          ...comment,
          id: '00000000-0000-4000-8000-000000000301',
          author_user_id: '00000000-0000-4000-8000-000000000999',
          author_display_name: 'Attricat',
          author_email: 'system+workspace@attricat.invalid',
          author_kind: 'system',
          body: 'A bulk edit stopped here.',
        },
        {
          ...comment,
          id: '00000000-0000-4000-8000-000000000302',
          author_user_id: userId,
          author_display_name: 'Reader',
          author_email: 'reader@example.com',
          author_kind: 'person',
          body: 'My note.',
        },
      ],
    });
    render(
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <RecordCommentsPanel recordId="record" />
      </QueryClientProvider>,
    );
    const system = (
      await screen.findByText('A bulk edit stopped here.')
    ).closest('li') as HTMLElement;
    expect(system.textContent).toContain('Attricat');
    expect(system.textContent).toContain('System');
    expect(system.querySelector('img')).not.toBeNull();
    expect(system.querySelector('.MuiAvatar-root')).toBeNull();
    expect(system.querySelector('button')).toBeNull();
    const own = screen.getByText('My note.').closest('li') as HTMLElement;
    expect(own.textContent).not.toContain('System');
    expect(own.querySelector('.MuiAvatar-root')).not.toBeNull();
  });
});

describe('record comment order', () => {
  it('deletes an own comment after confirmation', async () => {
    const own = {
      ...comment,
      id: '00000000-0000-4000-8000-000000000303',
      author_user_id: userId,
      author_display_name: 'Reader',
      author_email: 'reader@example.com',
      author_kind: 'person' as const,
      body: 'Typo here.',
      revision: 2,
    };
    vi.mocked(listComments)
      .mockResolvedValueOnce({ has_more: false, items: [own] })
      .mockResolvedValue({ has_more: false, items: [] });
    vi.mocked(deleteComment).mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <RecordCommentsPanel recordId="record" />
      </QueryClientProvider>,
    );
    const item = (await screen.findByText('Typo here.')).closest(
      'li',
    ) as HTMLElement;
    await user.click(within(item).getByRole('button', { name: 'Delete' }));
    expect(deleteComment).not.toHaveBeenCalled();
    const dialog = screen.getByRole('dialog', { name: 'Delete comment?' });
    await user.click(within(dialog).getByRole('button', { name: 'Delete' }));
    expect(deleteComment).toHaveBeenCalledWith('record', own.id, 2);
    expect(await screen.findByText('Comment deleted.')).toBeTruthy();
    await waitFor(() => expect(screen.queryByText('Typo here.')).toBeNull());
  });

  it('lists comments oldest first with earlier pages above and the composer below', async () => {
    const person = {
      ...comment,
      author_user_id: userId,
      author_display_name: 'Reader',
      author_email: 'reader@example.com',
      author_kind: 'person' as const,
    };
    // The API pages comments newest first.
    vi.mocked(listComments).mockResolvedValue({
      has_more: true,
      items: [
        {
          ...person,
          id: '00000000-0000-4000-8000-000000000402',
          body: 'Newest note.',
        },
        {
          ...person,
          id: '00000000-0000-4000-8000-000000000401',
          body: 'Older note.',
        },
      ],
    });
    render(
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <RecordCommentsPanel recordId="record" />
      </QueryClientProvider>,
    );

    const older = await screen.findByText('Older note.');
    const newest = screen.getByText('Newest note.');
    const earlier = screen.getByRole('button', {
      name: 'Show earlier comments',
    });
    const composer = await screen.findByRole('textbox', { name: /Comment/ });
    const follows = (first: Node, second: Node) =>
      Boolean(
        first.compareDocumentPosition(second) &
        Node.DOCUMENT_POSITION_FOLLOWING,
      );
    expect(follows(earlier, older)).toBe(true);
    expect(follows(older, newest)).toBe(true);
    expect(follows(newest, composer)).toBe(true);
  });
});
