// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { forwardRef, type ComponentPropsWithoutRef } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import i18n from '../../i18n';
import { currentSession } from '../auth/api';
import { listWorkflowRuns, listWorkflows } from './api';
import { WorkflowsPage } from './WorkflowsPage';

vi.mock('@tanstack/react-router', () => ({
  Link: forwardRef<
    HTMLAnchorElement,
    ComponentPropsWithoutRef<'a'> & { params?: object; to: string }
  >(({ children, params, to, ...props }, ref) => {
    void params;
    return (
      <a {...props} href={to} ref={ref}>
        {children}
      </a>
    );
  }),
}));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./api', () => ({ listWorkflowRuns: vi.fn(), listWorkflows: vi.fn() }));

describe('WorkflowsPage', () => {
  afterEach(async () => {
    vi.clearAllMocks();
    await i18n.changeLanguage('en');
  });

  it('shows the host-owned unauthorized state without loading workflow data', async () => {
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { workflows_manage: false, workflows_read: false },
    } as Awaited<ReturnType<typeof currentSession>>);
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });

    render(
      <QueryClientProvider client={client}>
        <WorkflowsPage />
      </QueryClientProvider>,
    );

    expect(
      await screen.findByText(
        'You are not authorized to view workflows in this workspace.',
      ),
    ).toBeDefined();
    expect(listWorkflows).not.toHaveBeenCalled();
    expect(listWorkflowRuns).not.toHaveBeenCalled();
  });

  it('shows localized statuses without management controls to read-only users', async () => {
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { workflows_manage: false, workflows_read: true },
    } as Awaited<ReturnType<typeof currentSession>>);
    vi.mocked(listWorkflows).mockResolvedValue([
      {
        code: 'example-workflow',
        compiled_plan: {},
        created_at: '2026-01-01T00:00:00Z',
        definition: 'format_version = 1',
        definition_hash: 'a'.repeat(64),
        enabled_version: null,
        id: '223e4567-e89b-12d3-a456-426614174000',
        manual_enabled: false,
        name: 'Example workflow',
        published_at: null,
        status: 'draft',
        version: 1,
      },
    ]);
    vi.mocked(listWorkflowRuns).mockResolvedValue([
      {
        attempts: 0,
        cancelled_at: null,
        causal_depth: 0,
        completed_at: null,
        created_at: '2026-01-01T00:00:00Z',
        failed_at: null,
        id: '123e4567-e89b-12d3-a456-426614174000',
        last_error: null,
        root_trigger_event_id: '423e4567-e89b-12d3-a456-426614174000',
        source: 'event',
        status: 'pending',
        trigger_event_id: '323e4567-e89b-12d3-a456-426614174000',
        trigger_sequence: 4,
        workflow_id: '223e4567-e89b-12d3-a456-426614174000',
        workflow_version: 1,
      },
    ]);
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });

    await i18n.changeLanguage('pl');
    render(
      <QueryClientProvider client={client}>
        <WorkflowsPage />
      </QueryClientProvider>,
    );

    expect(await screen.findByText('Szkic')).toBeDefined();
    expect(await screen.findByText('Oczekujący')).toBeDefined();
    expect(
      screen.queryByRole('button', { name: 'Nowy przepływ pracy' }),
    ).toBeNull();
  });
});
