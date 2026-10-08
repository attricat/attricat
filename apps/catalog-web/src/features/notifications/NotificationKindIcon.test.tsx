// @vitest-environment jsdom
import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { NotificationKindIcon } from './NotificationKindIcon';
import type { Notification } from './schemas';

const notification = (
  kind: string,
  data: Notification['data'] = {},
): Notification => ({
  id: '00000000-0000-4000-8000-000000000001',
  kind,
  title: 'Title',
  body: null,
  actor_user_id: null,
  actor_display_name: null,
  actor_email: null,
  subject: null,
  data,
  read: false,
  read_at: null,
  created_at: '2026-10-08T10:00:00Z',
});

const iconClass = (item: Notification) =>
  render(<NotificationKindIcon notification={item} />)
    .container.querySelector('svg')
    ?.getAttribute('class');

describe('NotificationKindIcon', () => {
  it.each([
    ['entity.assigned', {}, 'lucide-user-round'],
    ['entity.assigned', { team_name: 'Editors' }, 'lucide-users-round'],
    ['entity.commented', {}, 'lucide-message-square'],
    ['agent.run_failed', {}, 'lucide-bot'],
    ['workspace.invitation_accepted', {}, 'lucide-user-plus'],
    ['unknown.kind', {}, 'lucide-bell'],
  ])('draws %s %o with %s', (kind, data, expected) => {
    expect(iconClass(notification(kind, data))).toContain(expected);
  });
});
