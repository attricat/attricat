// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import '../../i18n';
import { getEntityChanges, getEntityForm } from './api';
import { EntityChangesPage } from './EntityChangesPage';

vi.mock('@tanstack/react-router', () => ({
  createLink: <T,>(component: T) => component,
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
}));
vi.mock('./api', () => ({
  getEntityChanges: vi.fn(),
  getEntityForm: vi.fn(),
}));
vi.mock('./components/EntitySchemaSubheader', () => ({
  EntitySchemaSubheader: () => null,
}));

const change = (code: string) => ({
  audit_event_id: code,
  occurred_at: '2026-09-09T12:00:00Z',
  actor_user_id: null,
  actor_display_name: null,
  actor_email: null,
  actor_avatar_file_id: null,
  executor_type: 'human',
  agent_run_id: null,
  approval_decision: null,
  approved_by_user_id: null,
  approved_by_display_name: null,
  approved_by_avatar_file_id: null,
  attribute_id: code,
  attribute_code: code,
  context_id: null,
  context_code: null,
  change_kind: 'replace' as const,
  before_value: null,
  after_value: code,
});

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(getEntityForm).mockResolvedValue({
    blueprint: { blueprint: { name: 'Product' } },
  } as Awaited<ReturnType<typeof getEntityForm>>);
});

it('loads successive pages of entity changes without discarding prior changes', async () => {
  const entityId = '00000000-0000-4000-8000-000000000001';
  vi.mocked(getEntityChanges)
    .mockResolvedValueOnce({ items: [change('title')], next_offset: 25 })
    .mockResolvedValueOnce({ items: [change('subtitle')], next_offset: null });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <EntityChangesPage entityId={entityId} />
    </QueryClientProvider>,
  );
  expect(await screen.findByText('title', { selector: 'strong' })).toBeTruthy();
  expect(screen.getByText('Back to record')).toBeTruthy();
  await userEvent.click(screen.getByRole('button', { name: 'Load more' }));
  await waitFor(() =>
    expect(getEntityChanges).toHaveBeenCalledWith(entityId, 25),
  );
  expect(
    await screen.findByText('subtitle', { selector: 'strong' }),
  ).toBeTruthy();
  expect(screen.getByText('title', { selector: 'strong' })).toBeTruthy();
  expect(screen.queryByRole('button', { name: 'Load more' })).toBeNull();
});
