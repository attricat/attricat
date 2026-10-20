// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import {
  acknowledgeFinding,
  listFindings,
  listRecordRuleRuns,
  listRules,
  recheckRecord,
  type Finding,
  type Rule,
  type RuleRun,
} from './api';
import { RecordFindingsPanel } from './RecordFindingsPanel';

vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./api', () => ({
  acknowledgeFinding: vi.fn(),
  listFindings: vi.fn(),
  listRecordRuleRuns: vi.fn(),
  listRules: vi.fn(),
  recheckRecord: vi.fn(),
}));

const recordId = '00000000-0000-4000-8000-000000000001';
const blueprintId = '00000000-0000-4000-8000-000000000002';
const ruleId = '00000000-0000-4000-8000-000000000003';
const manager = {
  capabilities: { rules_read: true, rules_manage: true },
} as Awaited<ReturnType<typeof currentSession>>;
const rule: Rule = {
  id: ruleId,
  blueprint_id: blueprintId,
  blueprint_version: 1,
  context_id: null,
  code: 'title_required',
  name: 'Title required',
  version: 1,
  status: 'published',
  definition: '',
  definition_hash: '',
  compiled_plan: {},
  published_at: null,
  created_at: '',
  enabled_version: 1,
};
const finding: Finding = {
  id: '00000000-0000-4000-8000-000000000004',
  rule_id: ruleId,
  rule_version: 1,
  record_id: recordId,
  context_id: null,
  severity: 'error',
  message: 'title is required',
  evidence: { contexts: ['default', 'pl'] },
  state: 'open',
  acknowledged_at: null,
  resolved_at: null,
  created_at: '2026-10-20T10:00:00Z',
  updated_at: '2026-10-20T10:00:00Z',
};
const run = (status: string, completedAt: string | null): RuleRun => ({
  id: '00000000-0000-4000-8000-000000000005',
  rule_id: ruleId,
  rule_version: 1,
  source: 'manual',
  dry_run: false,
  scope_record_id: recordId,
  status,
  candidate_cursor: null,
  candidates_evaluated: 0,
  findings_created: 0,
  findings_resolved: 0,
  attempts: 0,
  last_error: null,
  completed_at: completedAt,
  created_at: '2026-10-20T10:00:00Z',
});

const renderPanel = () =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <RecordFindingsPanel
        blueprintId={blueprintId}
        blueprintVersion={1}
        recordId={recordId}
      />
    </QueryClientProvider>,
  );

describe('RecordFindingsPanel', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRules).mockResolvedValue([rule]);
    vi.mocked(listFindings).mockResolvedValue([finding]);
    vi.mocked(listRecordRuleRuns).mockResolvedValue([]);
  });

  it('lists the record’s unresolved findings with their rule and contexts', async () => {
    const user = userEvent.setup();
    vi.mocked(acknowledgeFinding).mockResolvedValue({
      ...finding,
      state: 'acknowledged',
    });
    renderPanel();
    const panel = await screen.findByRole('region', { name: 'Data quality' });
    const item = await within(panel).findByRole('listitem');
    expect(within(item).getByText('title is required')).toBeTruthy();
    expect(within(item).getByText(/Title required v1/)).toBeTruthy();
    expect(within(item).getByText(/Contexts: default, pl/)).toBeTruthy();
    await user.click(within(item).getByRole('button', { name: 'Acknowledge' }));
    expect(vi.mocked(acknowledgeFinding).mock.calls[0]?.[0]).toBe(finding.id);
  });

  it('re-runs the record’s checks and refreshes findings when they finish', async () => {
    const user = userEvent.setup();
    vi.mocked(recheckRecord).mockResolvedValue({ run_ids: [] });
    renderPanel();
    await screen.findByText('title is required');
    vi.mocked(listRecordRuleRuns)
      .mockResolvedValueOnce([run('pending', null)])
      .mockResolvedValue([run('completed', '2026-10-20T10:01:00Z')]);
    vi.mocked(listFindings).mockResolvedValue([
      { ...finding, state: 'resolved' },
    ]);
    await user.click(screen.getByRole('button', { name: 'Re-run checks' }));
    expect(recheckRecord).toHaveBeenCalledWith(recordId);
    await waitFor(() =>
      expect(screen.getByText(/No open findings\. Last checked/)).toBeTruthy(),
    );
    expect(screen.queryByText('title is required')).toBeNull();
  });

  it('renders nothing when no rule applies and nothing is reported', async () => {
    vi.mocked(listRules).mockResolvedValue([
      { ...rule, enabled_version: null },
    ]);
    vi.mocked(listFindings).mockResolvedValue([]);
    const { container } = renderPanel();
    await waitFor(() => expect(listFindings).toHaveBeenCalled());
    expect(container.textContent).toBe('');
  });
});
