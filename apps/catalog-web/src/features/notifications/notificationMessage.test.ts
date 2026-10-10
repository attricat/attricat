import { describe, expect, it } from 'vitest';
import i18n from '../../i18n';
import { notificationMessage } from './notificationMessage';
import type { Notification } from './schemas';

const notification = (overrides: Partial<Notification>): Notification => ({
  id: '00000000-0000-4000-8000-000000000001',
  kind: 'record.assigned',
  title: 'Ada assigned you to a Task record',
  body: null,
  actor_user_id: '00000000-0000-4000-8000-000000000002',
  actor_display_name: 'Ada',
  actor_email: 'ada@example.test',
  subject: { kind: 'record', id: '00000000-0000-4000-8000-000000000003' },
  data: { blueprint_name: 'Task' },
  read: false,
  read_at: null,
  created_at: '2026-10-08T10:00:00Z',
  ...overrides,
});

describe('notificationMessage', () => {
  const t = i18n.getFixedT('en');

  it('names a readable record by label and otherwise by blueprint', () => {
    expect(notificationMessage(t, notification({}), 'Quarterly review')).toBe(
      'Ada assigned you to “Quarterly review”',
    );
    expect(notificationMessage(t, notification({}))).toBe(
      'Ada assigned you to a Task record',
    );
    expect(
      notificationMessage(
        t,
        notification({ data: { blueprint_name: 'Task', team_name: 'QA' } }),
      ),
    ).toBe('Ada assigned your team QA to a Task record');
  });

  it('translates each known kind and falls back to the stored title', async () => {
    expect(
      notificationMessage(
        t,
        notification({
          kind: 'agent.approval_required',
          actor_display_name: null,
          actor_email: null,
          data: { conversation_title: 'Prices' },
        }),
      ),
    ).toBe('The agent is waiting for your approval in “Prices”');
    expect(
      notificationMessage(
        t,
        notification({
          kind: 'team.member_added',
          actor_display_name: null,
          data: { team_name: 'QA' },
        }),
      ),
    ).toBe('ada@example.test added you to the team QA');
    expect(
      notificationMessage(
        t,
        notification({ kind: 'extension.custom', title: 'Export ready' }),
      ),
    ).toBe('Export ready');
    await i18n.changeLanguage('pl');
    expect(
      notificationMessage(
        i18n.t,
        notification({ kind: 'record.commented' }),
        'Przegląd',
      ),
    ).toBe('Nowy komentarz do „Przegląd” (od: Ada)');
    await i18n.changeLanguage('en');
  });
});
