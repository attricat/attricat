// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { forwardRef, type ComponentPropsWithoutRef } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import {
  listWorkflowRevisions,
  listWorkflowRuns,
  publishWorkflowRevision,
  replayWorkflowRun,
} from './api';
import { workflowQueryKeys } from './query-keys';
import { WorkflowDetailPage } from './WorkflowDetailPage';

vi.mock('@tanstack/react-router', () => ({
  Link: forwardRef<
    HTMLAnchorElement,
    ComponentPropsWithoutRef<'a'> & { params?: object; to: string }
  >(({ children, params: _params, to, ...props }, ref) => (
    <a {...props} href={to} ref={ref}>
      {children}
    </a>
  )),
}));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./api', () => ({
  disableWorkflow: vi.fn(),
  enableWorkflowRevision: vi.fn(),
  listWorkflowRevisions: vi.fn(),
  listWorkflowRuns: vi.fn(),
  publishWorkflowRevision: vi.fn(),
  replayWorkflowRun: vi.fn(),
}));

const workflow = {
  code: 'example-workflow',
  compiled_plan: {},
  created_at: '2026-01-01T00:00:00Z',
  definition: 'format_version = 1',
  definition_hash: 'a'.repeat(64),
  enabled_version: null,
  id: '223e4567-e89b-12d3-a456-426614174000',
  name: 'Example workflow',
  published_at: null,
  status: 'draft' as const,
  version: 1,
};

const run = {
  attempts: 5,
  cancelled_at: null,
  causal_depth: 0,
  completed_at: null,
  created_at: '2026-01-01T00:00:00Z',
  failed_at: '2026-01-01T00:00:00Z',
  id: '123e4567-e89b-12d3-a456-426614174000',
  last_error: 'A safe error',
  root_trigger_event_id: '423e4567-e89b-12d3-a456-426614174000',
  status: 'dead_letter' as const,
  trigger_event_id: '323e4567-e89b-12d3-a456-426614174000',
  trigger_sequence: 4,
  workflow_id: workflow.id,
  workflow_version: 1,
};

const renderPage = () => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <WorkflowDetailPage workflowId={workflow.id} />
    </QueryClientProvider>,
  );
  return client;
};

describe('WorkflowDetailPage', () => {
  afterEach(() => vi.clearAllMocks());

  it('keeps lifecycle controls unavailable to read-only users and associates tabs with panels', async () => {
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { workflows_manage: false, workflows_read: true },
    } as Awaited<ReturnType<typeof currentSession>>);
    vi.mocked(listWorkflowRevisions).mockResolvedValue([workflow]);
    vi.mocked(listWorkflowRuns).mockResolvedValue([run]);

    renderPage();

    expect(
      await screen.findByText(
        'You have read-only workflow access. Management actions are unavailable.',
      ),
    ).toBeDefined();
    expect(screen.queryByRole('button', { name: 'New revision' })).toBeNull();
    const revisionsTab = screen.getByRole('tab', { name: 'Revision history' });
    expect(revisionsTab.getAttribute('aria-controls')).toBe(
      'workflow-tabpanel-0',
    );
    expect(screen.getByRole('tabpanel').getAttribute('aria-labelledby')).toBe(
      revisionsTab.id,
    );
  });

  it('invalidates lifecycle and replay diagnostics after management actions', async () => {
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { workflows_manage: true, workflows_read: true },
    } as Awaited<ReturnType<typeof currentSession>>);
    vi.mocked(listWorkflowRevisions).mockResolvedValue([workflow]);
    vi.mocked(listWorkflowRuns).mockResolvedValue([run]);
    vi.mocked(publishWorkflowRevision).mockResolvedValue(workflow);
    vi.mocked(replayWorkflowRun).mockResolvedValue();
    const client = renderPage();
    const invalidateQueries = vi.spyOn(client, 'invalidateQueries');
    const user = userEvent.setup();

    await user.click(await screen.findByRole('button', { name: 'Publish' }));
    expect(publishWorkflowRevision).toHaveBeenCalledWith(workflow.id, 1);
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: workflowQueryKeys.revisions(workflow.id),
    });

    await user.click(screen.getByRole('tab', { name: 'Run diagnostics' }));
    await user.click(await screen.findByRole('button', { name: /Replay/ }));
    expect(replayWorkflowRun).toHaveBeenCalledWith(run.id);
    expect(invalidateQueries).toHaveBeenCalledWith({
      queryKey: workflowQueryKeys.runs(),
    });
  });
});
