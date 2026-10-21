// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { RecordCommentsPanel } from './RecordCommentsPanel';
import { listComments } from './api';

const userId = '00000000-0000-4000-8000-000000000201';

vi.mock(
  '@tanstack/react-router',
  async () => (await import('../../test/routerLinkMock')).routerLinkMock,
);
vi.mock('./api', () => ({
  listComments: vi.fn(),
  createComment: vi.fn(),
  updateComment: vi.fn(),
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
