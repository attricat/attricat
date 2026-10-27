// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import {
  acknowledgeFinding,
  listRecordRuleRuns,
  listRules,
  listUnresolvedRecordFindings,
  recheckRecord,
  type Finding,
  type Rule,
  type RuleRun,
} from './api';
import { RECORD_RULE_RUN_POLL_MS } from './constants';
import { RevealFieldContext } from '../views/fieldReveal';
import { RecordFindingsPanel } from './RecordFindingsPanel';
import { watchRecordRuleChecks } from './recordRuleChecks';
import { recordQueryKeys } from '../records/queryKeys';

vi.mock('@tanstack/react-router', () => ({
  Link: ({
    children,
    params,
  }: {
    children: ReactNode;
    params: { ruleId: string; version: string };
  }) => (
    <a href={`/manage/rules/${params.ruleId}/revisions/${params.version}`}>
      {children}
    </a>
  ),
}));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./api', () => ({
  acknowledgeFinding: vi.fn(),
  listRecordRuleRuns: vi.fn(),
  listRules: vi.fn(),
  listUnresolvedRecordFindings: vi.fn(),
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
const run = (
  status: RuleRun['status'],
  completedAt: string | null,
): RuleRun => ({
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

const newClient = () =>
  new QueryClient({ defaultOptions: { queries: { retry: false } } });

/** Caches what the server rewrites beside findings: tags and data quality. */
const seedRecordState = (client: QueryClient) => {
  const keys = [
    recordQueryKeys.form(recordId),
    recordQueryKeys.preview(recordId),
    recordQueryKeys.resolvedPreview(recordId, undefined),
    recordQueryKeys.search({ blueprint: 'products' }),
    recordQueryKeys.systemTagFacet({ blueprint: { code: 'products' } }),
  ];
  for (const key of keys) client.setQueryData(key, {});
  return () =>
    keys.map((queryKey) => client.getQueryState(queryKey)?.isInvalidated);
};

const renderPanel = (
  props: Partial<Parameters<typeof RecordFindingsPanel>[0]> = {},
  revealField: ((code: string) => void) | null = null,
  client: QueryClient = newClient(),
) =>
  render(
    <QueryClientProvider client={client}>
      <RevealFieldContext value={revealField}>
        <RecordFindingsPanel
          blueprintId={blueprintId}
          blueprintVersion={1}
          recordId={recordId}
          {...props}
        />
      </RevealFieldContext>
    </QueryClientProvider>,
  );

describe('RecordFindingsPanel', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRules).mockResolvedValue([rule]);
    vi.mocked(listUnresolvedRecordFindings).mockResolvedValue({
      items: [finding],
    });
    vi.mocked(listRecordRuleRuns).mockResolvedValue([]);
  });

  it('lists the record’s unresolved findings with their rule and contexts', async () => {
    const user = userEvent.setup();
    vi.mocked(acknowledgeFinding).mockResolvedValue({
      ...finding,
      state: 'acknowledged',
    });
    const client = newClient();
    const invalidated = seedRecordState(client);
    renderPanel({}, null, client);
    const item = await screen.findByRole('listitem');
    expect(within(item).getByText('title is required')).toBeTruthy();
    expect(
      within(item)
        .getByRole('link', { name: 'Title required' })
        .getAttribute('href'),
    ).toBe(`/manage/rules/${ruleId}/revisions/1`);
    expect(within(item).getByText(/Contexts: default, pl/)).toBeTruthy();
    await user.click(within(item).getByRole('button', { name: 'Acknowledge' }));
    expect(vi.mocked(acknowledgeFinding).mock.calls[0]?.[0]).toBe(finding.id);
    await waitFor(() =>
      expect(invalidated()).toEqual([true, true, true, true, true]),
    );
  });

  it('keeps the server’s severity order and names fields by their labels', async () => {
    vi.mocked(listUnresolvedRecordFindings).mockResolvedValue({
      items: [
        {
          ...finding,
          message: "'title' is required",
          evidence: { contexts: ['default', 'pl'], attribute_code: 'title' },
        },
        {
          ...finding,
          id: '00000000-0000-4000-8000-000000000006',
          severity: 'info',
          message: "'summary' is required",
          evidence: { contexts: ['pl'], attribute_code: 'summary' },
        },
      ],
    });
    renderPanel({
      attributes: [
        { code: 'title', name: 'Listing title' },
        { code: 'summary' },
      ],
      contextCodes: ['default', 'pl'],
    });
    await screen.findByText('Listing title is required');
    const [first, second] = screen.getAllByRole('listitem');
    expect(within(first!).getByText('Error')).toBeTruthy();
    expect(within(first!).getByText(/All contexts/)).toBeTruthy();
    expect(within(second!).getByText('summary is required')).toBeTruthy();
    expect(within(second!).getByText(/Contexts: pl/)).toBeTruthy();
  });

  it('leads to the field a finding is about', async () => {
    const user = userEvent.setup();
    const revealField = vi.fn();
    vi.mocked(listUnresolvedRecordFindings).mockResolvedValue({
      items: [
        {
          ...finding,
          message: "'title' is required",
          evidence: { attribute_code: 'title' },
        },
        {
          ...finding,
          id: '00000000-0000-4000-8000-000000000006',
          message: 'Too short for search results',
          evidence: { attribute_code: 'summary' },
        },
      ],
    });
    renderPanel(
      {
        attributes: [
          { code: 'title', name: 'Listing title' },
          { code: 'summary', name: 'Summary' },
        ],
      },
      revealField,
    );
    const [named, unnamed] = await screen.findAllByRole('listitem');
    expect(named!.textContent).toContain('Listing title is required');
    expect(unnamed!.textContent).toContain(
      'Too short for search results (field: Summary)',
    );

    await user.click(
      within(named!).getByRole('button', { name: 'Listing title' }),
    );
    await user.click(within(unnamed!).getByRole('button', { name: 'Summary' }));

    expect(revealField.mock.calls).toEqual([['title'], ['summary']]);
  });

  it('re-runs the record’s checks and refreshes findings when they finish', async () => {
    const user = userEvent.setup();
    vi.mocked(recheckRecord).mockResolvedValue({ run_ids: [] });
    const client = newClient();
    renderPanel({}, null, client);
    await screen.findByText('title is required');
    vi.mocked(listRecordRuleRuns)
      .mockResolvedValueOnce([run('pending', null)])
      .mockResolvedValue([run('completed', '2026-10-20T10:01:00Z')]);
    vi.mocked(listUnresolvedRecordFindings).mockResolvedValue({ items: [] });
    await user.click(screen.getByRole('button', { name: 'Re-run checks' }));
    expect(recheckRecord).toHaveBeenCalledWith(recordId);
    const invalidated = seedRecordState(client);
    // The finished run arrives with the next poll, a second later.
    await waitFor(
      () =>
        expect(
          screen.getByText(/No open findings\. Last checked/),
        ).toBeTruthy(),
      { timeout: 3 * RECORD_RULE_RUN_POLL_MS },
    );
    expect(screen.queryByText('title is required')).toBeNull();
    expect(invalidated()).toEqual([true, true, true, true, true]);
  });

  it('loads further pages of findings on request', async () => {
    const user = userEvent.setup();
    const older = {
      ...finding,
      id: '00000000-0000-4000-8000-000000000006',
      message: 'summary is too long',
    };
    vi.mocked(listUnresolvedRecordFindings).mockImplementation(
      (_, offset = 0) =>
        Promise.resolve(
          offset === 0
            ? { items: [finding], nextOffset: 20 }
            : { items: [older] },
        ),
    );
    renderPanel();
    await screen.findByText('title is required');
    await user.click(
      screen.getByRole('button', { name: 'Load more findings' }),
    );
    expect(await screen.findByText('summary is too long')).toBeTruthy();
    expect(listUnresolvedRecordFindings).toHaveBeenLastCalledWith(recordId, 20);
    expect(screen.queryByRole('button', { name: 'Load more findings' })).toBe(
      null,
    );
  });

  it('says when no rule applies and nothing is reported', async () => {
    vi.mocked(listRules).mockResolvedValue([
      { ...rule, enabled_version: null },
    ]);
    vi.mocked(listUnresolvedRecordFindings).mockResolvedValue({ items: [] });
    renderPanel();
    expect(await screen.findByText('No open findings.')).toBeTruthy();
    expect(
      await screen.findByText(
        'No enabled rule checks records of this blueprint.',
      ),
    ).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Re-run checks' })).toBeNull();
  });

  it('waits for an edit’s rule runs only when an enabled rule applies', async () => {
    const pollsAfterEdit = async () => {
      vi.mocked(listRecordRuleRuns).mockClear();
      watchRecordRuleChecks(recordId);
      const { unmount } = renderPanel();
      await waitFor(() => expect(listRecordRuleRuns).toHaveBeenCalled());
      await new Promise((resolve) =>
        setTimeout(resolve, 2.5 * RECORD_RULE_RUN_POLL_MS),
      );
      unmount();
      return vi.mocked(listRecordRuleRuns).mock.calls.length;
    };
    expect(await pollsAfterEdit()).toBeGreaterThan(1);

    vi.mocked(listRules).mockResolvedValue([
      { ...rule, enabled_version: null },
    ]);
    expect(await pollsAfterEdit()).toBe(1);
  }, 10_000);
});
