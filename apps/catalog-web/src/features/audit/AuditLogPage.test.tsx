// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import i18n from '../../i18n';
import type { AuditEvent } from './api';
import { listAuditEvents } from './api';
import { AuditLogPage } from './AuditLogPage';

vi.mock('./api', () => ({
  listAuditEvents: vi.fn(),
}));

const event: AuditEvent = {
  action: 'entity.updated',
  actor_display_name: 'Ada Lovelace',
  actor_email: null,
  actor_user_id: '00000000-0000-4000-8000-000000000001',
  agent_conversation_id: null,
  agent_run_id: null,
  agent_tool_call_id: null,
  agent_tool_name: null,
  approval_decision: null,
  approved_by_display_name: null,
  approved_by_email: null,
  approved_by_user_id: null,
  authorization_scope: {},
  correlation_id: '00000000-0000-4000-8000-000000000002',
  executor_type: 'human',
  id: '00000000-0000-4000-8000-000000000003',
  metadata: {},
  occurred_at: '2026-03-06T12:00:00.000Z',
  outcome: 'success',
  request_id: '00000000-0000-4000-8000-000000000004',
  target: { type: 'entity' },
};

const renderPage = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <AuditLogPage />
    </QueryClientProvider>,
  );
};

describe('AuditLogPage', () => {
  beforeEach(() => {
    vi.mocked(listAuditEvents).mockReset();
  });

  it('opens event details with a labelled button by mouse and keyboard', async () => {
    await i18n.changeLanguage('en');
    vi.mocked(listAuditEvents).mockResolvedValue({
      events: [event],
      limit: 50,
      offset: 0,
      total: 1,
    });
    const user = userEvent.setup();

    renderPage();

    const detailsButton = await screen.findByRole('button', {
      name: 'View details for entity.updated by Ada Lovelace',
    });
    await user.click(detailsButton);
    expect(
      await screen.findByRole('heading', { name: 'Audit event' }),
    ).toBeTruthy();

    await user.keyboard('{Escape}');
    await waitFor(() =>
      expect(screen.queryByRole('heading', { name: 'Audit event' })).toBeNull(),
    );

    detailsButton.focus();
    await user.keyboard('{Enter}');
    expect(
      await screen.findByRole('heading', { name: 'Audit event' }),
    ).toBeTruthy();
  });

  it('waits to request text filters until they are applied', async () => {
    vi.mocked(listAuditEvents).mockResolvedValue({
      events: [],
      limit: 50,
      offset: 0,
      total: 0,
    });
    const user = userEvent.setup();

    renderPage();

    await waitFor(() => expect(listAuditEvents).toHaveBeenCalledTimes(1));
    await user.type(
      screen.getByRole('textbox', { name: 'Action category' }),
      'catalog',
    );
    expect(listAuditEvents).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole('button', { name: 'Apply filters' }));
    await waitFor(() =>
      expect(listAuditEvents).toHaveBeenLastCalledWith({
        action_category: 'catalog',
        limit: 50,
        offset: 0,
      }),
    );
  });

  it('uses the resolved locale for dates and translates Polish fallback labels', async () => {
    await i18n.changeLanguage('pl');
    vi.mocked(listAuditEvents).mockResolvedValue({
      events: [
        {
          ...event,
          actor_display_name: null,
          actor_email: null,
          actor_user_id: null,
          target: {},
        },
      ],
      limit: 50,
      offset: 0,
      total: 1,
    });

    renderPage();

    const expectedDate = new Intl.DateTimeFormat('pl', {
      dateStyle: 'medium',
      timeStyle: 'medium',
    }).format(new Date(event.occurred_at));
    expect(await screen.findByText(expectedDate)).toBeTruthy();
    expect(screen.getByText('Obszar roboczy')).toBeTruthy();
    expect(screen.getByText('System')).toBeTruthy();

    await i18n.changeLanguage('en');
  });

  it('translates fallback labels in English', async () => {
    await i18n.changeLanguage('en');
    vi.mocked(listAuditEvents).mockResolvedValue({
      events: [
        {
          ...event,
          actor_display_name: null,
          actor_email: null,
          actor_user_id: null,
          target: {},
        },
      ],
      limit: 50,
      offset: 0,
      total: 1,
    });

    renderPage();

    expect(await screen.findByText('System')).toBeTruthy();
    expect(screen.getByText('Workspace')).toBeTruthy();
  });
});
