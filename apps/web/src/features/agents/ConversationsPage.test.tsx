// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ListItemButton } from '@mui/material';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { searchConversations } from './api';
import { ConversationsPage } from './ConversationsPage';

const navigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  Link: ({
    children,
    style,
    ...props
  }: {
    children: React.ReactNode;
    style?: React.CSSProperties;
  }) => (
    <a href="/agents/one" style={style} {...props}>
      {children}
    </a>
  ),
  useNavigate: () => navigate,
}));
vi.mock('../../components/RouterLink', () => ({
  RouterButton: ({
    children,
    params,
  }: {
    children: React.ReactNode;
    params: { recordId: string };
  }) => <a href={`/records/${params.recordId}`}>{children}</a>,
  RouterListItemButton: ({
    children,
    sx,
  }: {
    children: React.ReactNode;
    sx: object;
  }) => (
    <ListItemButton component="a" href="/agents/one" sx={sx}>
      {children}
    </ListItemButton>
  ),
}));
vi.mock('./api', () => ({ searchConversations: vi.fn() }));

const conversation = (
  id: string,
  title: string,
  recordId: string | null = null,
) => ({
  id,
  title,
  title_source: 'generated',
  record_id: recordId,
  context_id: null,
  created_at: '2026-09-28T00:00:00Z',
  updated_at: '2026-09-28T00:00:00Z',
});
const renderPage = (q?: string, scope?: 'all') =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ConversationsPage search={{ q, scope }} />
    </QueryClientProvider>,
  );

describe('ConversationsPage', () => {
  beforeEach(() => {
    vi.mocked(searchConversations).mockReset();
    navigate.mockReset();
  });

  it('searches server-side and loads additional pages with full-width links', async () => {
    vi.mocked(searchConversations)
      .mockResolvedValueOnce({
        items: [conversation('one', 'Pricing review')] as never,
        next_cursor: 'cursor',
      })
      .mockResolvedValueOnce({
        items: [conversation('two', 'Product notes')] as never,
        next_cursor: null,
      });
    const user = userEvent.setup();
    renderPage('product');
    expect(await screen.findByText('Pricing review')).toBeTruthy();
    expect(searchConversations).toHaveBeenCalledWith('product', undefined, {
      scope: 'unattached',
    });
    const link = screen.getByText('Pricing review').closest('a');
    expect(link?.className).toContain('MuiListItemButton-root');
    expect(window.getComputedStyle(link!).width).toBe('100%');
    await user.click(screen.getByRole('button', { name: 'Load more' }));
    expect(await screen.findByText('Product notes')).toBeTruthy();
    expect(searchConversations).toHaveBeenCalledWith('product', 'cursor', {
      scope: 'unattached',
    });
    expect(screen.queryByRole('button', { name: 'Load more' })).toBeNull();
  });

  it('puts a new search in the route URL', async () => {
    vi.mocked(searchConversations).mockResolvedValue({
      items: [],
      next_cursor: null,
    });
    const user = userEvent.setup();
    renderPage();
    await user.type(
      screen.getByRole('textbox', { name: 'Search conversations' }),
      'pricing',
    );
    await user.click(screen.getByRole('button', { name: 'Search' }));
    expect(navigate).toHaveBeenCalledWith({
      to: '/agents',
      search: { q: 'pricing' },
    });
  });

  it('includes record conversations when asked and links their record', async () => {
    vi.mocked(searchConversations).mockResolvedValue({
      items: [conversation('one', 'Sizing question', 'record-1')] as never,
      next_cursor: null,
    });
    const user = userEvent.setup();
    renderPage(undefined, 'all');
    expect(await screen.findByText('Sizing question')).toBeTruthy();
    expect(searchConversations).toHaveBeenCalledWith('', undefined, {
      scope: 'all',
    });
    expect(
      screen.getByRole('link', { name: 'Open record' }).getAttribute('href'),
    ).toBe('/records/record-1');
    await user.click(
      screen.getByRole('switch', { name: 'Show record conversations' }),
    );
    expect(navigate).toHaveBeenCalledWith({
      to: '/agents',
      search: { q: undefined, scope: undefined },
    });
  });
});
