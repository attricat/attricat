// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import { getEntityLabels } from '../entities/api';
import {
  deleteNotification,
  listNotifications,
  markAllNotificationsRead,
  setNotificationRead,
} from './api';
import { InboxPage } from './InboxPage';
import type { Notification } from './schemas';

const navigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigate,
}));
vi.mock('../../components/RouterLink', () => ({
  RouterListItemButton: ({
    children,
    onClick,
  }: {
    children: React.ReactNode;
    onClick: () => void;
  }) => (
    <a href="#subject" onClick={onClick}>
      {children}
    </a>
  ),
}));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('../entities/api', () => ({ getEntityLabels: vi.fn() }));
vi.mock('./api', () => ({
  deleteNotification: vi.fn(),
  listNotifications: vi.fn(),
  markAllNotificationsRead: vi.fn(),
  setNotificationRead: vi.fn(),
}));

const entityId = '00000000-0000-4000-8000-000000000010';
const item = (id: string, overrides: Partial<Notification>): Notification => ({
  id,
  kind: 'entity.assigned',
  title: 'Ada assigned you to a Task record',
  body: null,
  actor_user_id: '00000000-0000-4000-8000-000000000002',
  actor_display_name: 'Ada',
  actor_email: 'ada@example.test',
  subject: { kind: 'entity', id: entityId },
  data: { blueprint_name: 'Task' },
  read: false,
  read_at: null,
  created_at: '2026-10-08T10:00:00Z',
  ...overrides,
});
const assigned = item('00000000-0000-4000-8000-000000000001', {});
const joined = item('00000000-0000-4000-8000-000000000003', {
  kind: 'team.member_added',
  subject: null,
  data: { team_name: 'QA' },
  read: true,
  read_at: '2026-10-07T10:00:00Z',
  created_at: '2026-10-07T09:00:00Z',
});

const renderPage = (filter: 'all' | 'unread' = 'all') =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <InboxPage filter={filter} />
    </QueryClientProvider>,
  );

describe('InboxPage', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(currentSession).mockResolvedValue({
      user_id: '00000000-0000-4000-8000-000000000020',
      workspace_id: '00000000-0000-4000-8000-000000000021',
    } as never);
    vi.mocked(getEntityLabels).mockResolvedValue({
      items: [
        {
          id: entityId,
          blueprint_code: 'task',
          display: { default: 'Review' },
        },
      ],
    } as never);
    vi.mocked(listNotifications).mockResolvedValue({
      items: [assigned, joined],
      has_more: false,
      unread_count: 1,
    });
    vi.mocked(setNotificationRead).mockResolvedValue(undefined);
    vi.mocked(deleteNotification).mockResolvedValue(undefined);
    vi.mocked(markAllNotificationsRead).mockResolvedValue({ updated: 1 });
  });

  it('lists translated notifications and links readable records by label', async () => {
    renderPage();
    expect(
      await screen.findByText('Ada assigned you to “Review”'),
    ).toBeTruthy();
    expect(screen.getByText('Ada added you to the team QA')).toBeTruthy();
    expect(screen.getByRole('img', { name: 'Unread' })).toBeTruthy();
    expect(screen.getByRole('tab', { name: 'Unread (1)' })).toBeTruthy();
    expect(listNotifications).toHaveBeenCalledWith(
      false,
      undefined,
      expect.anything(),
    );
  });

  it('opens a subject as read and changes, deletes and marks all', async () => {
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByText('Ada assigned you to “Review”'));
    expect(setNotificationRead).toHaveBeenCalledWith(assigned.id, true);

    const joinedRow = screen
      .getByText('Ada added you to the team QA')
      .closest('li')!;
    await user.click(
      within(joinedRow).getByRole('button', { name: 'Mark as unread' }),
    );
    expect(setNotificationRead).toHaveBeenCalledWith(joined.id, false);
    await user.click(
      within(joinedRow).getByRole('button', { name: 'Delete permanently' }),
    );
    expect(deleteNotification).toHaveBeenCalledWith(joined.id);

    await user.click(screen.getByRole('button', { name: 'Mark all as read' }));
    expect(markAllNotificationsRead).toHaveBeenCalledWith(assigned.created_at);
  });

  it('switches filters through the URL and explains an empty inbox', async () => {
    vi.mocked(listNotifications).mockResolvedValue({
      items: [],
      has_more: false,
      unread_count: 0,
    });
    const user = userEvent.setup();
    renderPage('unread');
    expect(await screen.findByText('No unread notifications')).toBeTruthy();
    expect(
      screen.getByRole('button', { name: 'Mark all as read' }),
    ).toHaveProperty('disabled', true);
    await user.click(screen.getByRole('tab', { name: 'All' }));
    await waitFor(() =>
      expect(navigate).toHaveBeenCalledWith({
        to: '/inbox',
        search: { filter: undefined },
      }),
    );
  });
});
