// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { forwardRef, type ComponentPropsWithoutRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import { listWorkflowRuns, listWorkflows } from './api';
import { WorkflowsPage } from './WorkflowsPage';

vi.mock('@tanstack/react-router', () => ({
  Link: forwardRef<
    HTMLAnchorElement,
    ComponentPropsWithoutRef<'a'> & { to: string }
  >(({ children, to, ...props }, ref) => (
    <a {...props} href={to} ref={ref}>
      {children}
    </a>
  )),
}));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./api', () => ({ listWorkflowRuns: vi.fn(), listWorkflows: vi.fn() }));

describe('WorkflowsPage', () => {
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
});
