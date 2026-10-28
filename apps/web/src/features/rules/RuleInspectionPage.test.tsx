// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { forwardRef, type ComponentPropsWithoutRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import { currentSession } from '../auth/api';
import {
  getBlueprintRevision,
  listBlueprintRevisions,
  listBlueprints,
  type Blueprint,
  type BlueprintWithAttributes,
} from '../blueprints/api';
import { listContexts, type AttributeContext } from '../contexts/api';
import { getRecordLabels } from '../records/api';
import {
  acknowledgeFinding,
  acknowledgeFindings,
  countFindings,
  disableRule,
  enableRule,
  listFindingsPage,
  listRuleRuns,
  listRules,
  runRuleNow,
  type Finding,
  type FindingsSearch,
  type RuleDetails,
  type RuleRun,
} from './api';
import { revisionStatuses } from '../../api/revisionStatus';
import { findingStates } from './constants';
import { RuleInspectionPage } from './RuleInspectionPage';

const navigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigate,
  Link: ({
    children,
    params,
    to,
    ...props
  }: ComponentPropsWithoutRef<'a'> & {
    params: { recordId: string };
    to: string;
  }) => (
    <a {...props} href={to.replace('$recordId', params.recordId)}>
      {children}
    </a>
  ),
  createLink: () =>
    forwardRef<HTMLAnchorElement, ComponentPropsWithoutRef<'a'>>(
      ({ children, ...props }, ref) => (
        <a {...props} ref={ref}>
          {children}
        </a>
      ),
    ),
}));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('../records/api', () => ({
  getRecordLabels: vi.fn(() => Promise.resolve({ items: [] })),
}));
vi.mock('../blueprints/api', () => ({
  getBlueprintRevision: vi.fn(),
  listBlueprintRevisions: vi.fn(() => Promise.resolve([])),
  listBlueprints: vi.fn(() => Promise.resolve([])),
}));
vi.mock('../contexts/api', () => ({
  listContexts: vi.fn(() => Promise.resolve([])),
}));
vi.mock('./api', () => ({
  acknowledgeFinding: vi.fn(),
  acknowledgeFindings: vi.fn(),
  countFindings: vi.fn(),
  disableRule: vi.fn(),
  enableRule: vi.fn(),
  listFindingsPage: vi.fn(),
  listRuleRuns: vi.fn(),
  listRules: vi.fn(),
  runRuleNow: vi.fn(),
}));

const id = '123e4567-e89b-42d3-a456-426614174000';
const manager = {
  capabilities: { rules_read: true, rules_manage: true },
} as Awaited<ReturnType<typeof currentSession>>;
const rule: RuleDetails = {
  id,
  blueprint_id: id,
  context_id: null,
  code: 'check',
  name: 'Check',
  version: 1,
  definition: '',
  definition_hash: '',
  compiled_plan: {},
  enabled: false,
  created_at: '',
  updated_at: '',
  not_applicable: [],
};
const finding: Finding = {
  id,
  rule_id: id,
  rule_version: 1,
  record_id: id,
  context_id: null,
  severity: 'warning',
  message: 'Check failed',
  evidence: {},
  state: 'open',
  acknowledged_at: null,
  resolved_at: null,
  created_at: '',
  updated_at: '',
};
const renderPage = (
  section: 'rules' | 'findings' | 'runs',
  findingsSearch?: FindingsSearch,
) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <RuleInspectionPage findingsSearch={findingsSearch} section={section} />
    </QueryClientProvider>,
  );
};

describe('RuleInspectionPage', () => {
  beforeEach(() => {
    vi.mocked(listRules).mockResolvedValue([rule]);
    vi.mocked(countFindings).mockResolvedValue({ count: 0 });
    vi.mocked(listBlueprints).mockResolvedValue([]);
  });
  afterEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it('disables both run variants while a run is pending', async () => {
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { rules_read: true, rules_manage: true },
    } as Awaited<ReturnType<typeof currentSession>>);
    vi.mocked(listRules).mockResolvedValue([{ ...rule, enabled: true }]);
    vi.mocked(runRuleNow).mockImplementation(() => new Promise(() => {}));
    renderPage('rules');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Dry run' }));
    expect(runRuleNow).toHaveBeenCalledWith(id, true);
    expect(
      (screen.getByRole('button', { name: 'Dry run' }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      (screen.getByRole('button', { name: 'Run now' }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it('disables acknowledgement while a finding is being updated', async () => {
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { rules_read: true, rules_manage: true },
    } as Awaited<ReturnType<typeof currentSession>>);
    vi.mocked(listFindingsPage).mockResolvedValue({
      items: [
        {
          id,
          rule_id: id,
          rule_version: 1,
          record_id: id,
          context_id: null,
          severity: 'warning',
          message: 'Check failed',
          evidence: {},
          state: 'open',
          acknowledged_at: null,
          resolved_at: null,
          created_at: '',
          updated_at: '',
        },
      ],
    });
    vi.mocked(acknowledgeFinding).mockImplementation(
      () => new Promise(() => {}),
    );
    renderPage('findings');
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole('button', { name: 'Acknowledge' }),
    );
    expect(acknowledgeFinding).toHaveBeenCalledWith(id, expect.any(Object));
    expect(
      (screen.getByRole('button', { name: 'Acknowledge' }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it('names the attribute a finding is about by its label', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listFindingsPage).mockResolvedValue({
      items: [
        {
          ...finding,
          message: "'title' is required",
          evidence: { attribute_code: 'title' },
        },
      ],
    });
    vi.mocked(getBlueprintRevision).mockResolvedValue({
      attributes: [{ code: 'title', name: 'Listing title' }],
    } as unknown as BlueprintWithAttributes);
    // Labels come from the latest published revision of the rule's
    // blueprint, which rules check, not from a newer draft.
    vi.mocked(listBlueprintRevisions).mockResolvedValue([
      { id, version: 4, status: revisionStatuses.draft },
      { id, version: 3, status: revisionStatuses.published },
    ] as Blueprint[]);
    renderPage('findings');
    expect(await screen.findByText('Listing title is required')).toBeTruthy();
    expect(vi.mocked(getBlueprintRevision).mock.calls[0]?.slice(0, 2)).toEqual([
      rule.blueprint_id,
      3,
    ]);
  });

  it('shows why a rule run was refused', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRules).mockResolvedValue([{ ...rule, enabled: true }]);
    vi.mocked(runRuleNow).mockRejectedValue(new Error('rule run refused'));
    renderPage('rules');
    await userEvent
      .setup()
      .click(await screen.findByRole('button', { name: 'Run now' }));
    expect((await screen.findByRole('alert')).textContent).toBe(
      'rule run refused',
    );
  });

  it('offers only a dry run of a disabled rule', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRules).mockResolvedValue([rule]);
    renderPage('rules');
    expect(await screen.findByRole('button', { name: 'Dry run' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Run now' })).toBeNull();
  });

  it('shows why an acknowledgement was refused', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listFindingsPage).mockResolvedValue({ items: [finding] });
    vi.mocked(acknowledgeFinding).mockRejectedValue(
      new Error('finding was already resolved'),
    );
    renderPage('findings');
    await userEvent
      .setup()
      .click(await screen.findByRole('button', { name: 'Acknowledge' }));
    expect((await screen.findByRole('alert')).textContent).toBe(
      'finding was already resolved',
    );
  });

  it('enables a rule from its switch', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRules).mockResolvedValue([rule]);
    vi.mocked(enableRule).mockResolvedValue({ ...rule, enabled: true });
    renderPage('rules');
    await userEvent
      .setup()
      .click(await screen.findByRole('switch', { name: 'Enable Check' }));
    expect(enableRule).toHaveBeenCalledWith(id, undefined);
  });

  it('lists each rule once, with its revisions it skips, and disables it', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    const other = '523e4567-e89b-42d3-a456-426614174000';
    vi.mocked(listRules).mockResolvedValue([
      {
        ...rule,
        enabled: true,
        version: 3,
        not_applicable: [
          {
            blueprint_version: 2,
            reason: 'attribute_missing',
            message: "unknown attribute 'title'",
          },
        ],
      },
      { ...rule, id: other, code: 'other', name: 'Other' },
    ]);
    vi.mocked(listBlueprints).mockResolvedValueOnce([
      { id, kind: 'record', name: '{{Product}}', version: 3 } as Blueprint,
    ]);
    vi.mocked(disableRule).mockResolvedValue(rule);
    renderPage('rules');
    const enabled = await screen.findByRole('switch', { name: 'Enable Check' });
    expect((enabled as HTMLInputElement).checked).toBe(true);
    const [, checkRow, otherRow] = screen.getAllByRole('row');
    expect(within(checkRow).getByText('v3')).toBeTruthy();
    expect(await within(checkRow).findByText('Product')).toBeTruthy();
    expect(
      within(checkRow).getByRole('img', {
        name: 'Skips the records of blueprint revision 2, which it does not fit',
      }),
    ).toBeTruthy();
    expect(within(otherRow).getByText('Disabled')).toBeTruthy();
    expect(within(otherRow).queryByRole('img')).toBeNull();

    await userEvent.setup().click(enabled);
    expect(disableRule).toHaveBeenCalledWith(id);
  });

  it('offers to accept the existing violations of an enforcing rule', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRules).mockResolvedValue([rule]);
    vi.mocked(enableRule)
      .mockRejectedValueOnce(
        new ApiRequestError(
          409,
          'the latest dry run found 3 existing violations',
          'rule_has_existing_violations',
          { existing_violations: 3 },
        ),
      )
      .mockResolvedValueOnce({ ...rule, enabled: true });
    renderPage('rules');
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole('switch', { name: 'Enable Check' }),
    );
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain(
      'Records already violate this rule (3).',
    );
    await user.click(
      within(alert).getByRole('button', { name: 'Enable anyway' }),
    );
    expect(enableRule).toHaveBeenLastCalledWith(id, true);
  });

  it('names the rule revision and links the record of each finding', async () => {
    const recordId = '223e4567-e89b-42d3-a456-426614174000';
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listFindingsPage).mockResolvedValue({
      items: [{ ...finding, record_id: recordId }],
    });
    vi.mocked(getRecordLabels).mockResolvedValue({
      items: [
        {
          id: recordId,
          blueprint_code: 'product',
          display: { default: 'Laptop' },
        },
      ],
    });
    renderPage('findings');
    const row = (await screen.findByText('Check failed')).closest('tr')!;
    expect(await within(row).findByText('Check v1')).toBeTruthy();
    const link = await within(row).findByRole('link', { name: 'Laptop' });
    expect(link.getAttribute('href')).toBe(`/records/${recordId}`);
    expect(getRecordLabels).toHaveBeenCalledWith(
      [recordId],
      expect.any(AbortSignal),
    );
  });

  it('counts the findings and acknowledges the selected or all open ones', async () => {
    const user = userEvent.setup();
    const second = { ...finding, id: '323e4567-e89b-42d3-a456-426614174000' };
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listFindingsPage).mockResolvedValue({ items: [finding, second] });
    vi.mocked(countFindings).mockResolvedValue({ count: 7 });
    vi.mocked(acknowledgeFindings).mockResolvedValue({ acknowledged: 1 });
    renderPage('findings');
    expect(await screen.findByText('7 findings')).toBeTruthy();
    const [first] = await screen.findAllByRole('checkbox', {
      name: /Select the finding of/,
    });
    await user.click(first!);
    await user.click(
      screen.getByRole('button', { name: 'Acknowledge selected (1)' }),
    );
    await waitFor(() =>
      expect(vi.mocked(acknowledgeFindings).mock.calls[0]?.[0]).toEqual({
        findingIds: [finding.id],
      }),
    );
    await user.click(
      screen.getByRole('button', { name: 'Acknowledge all 7 open' }),
    );
    const dialog = await screen.findByRole('dialog', {
      name: 'Acknowledge open findings?',
    });
    await user.click(
      within(dialog).getByRole('button', { name: 'Acknowledge all 7 open' }),
    );
    await waitFor(() =>
      expect(vi.mocked(acknowledgeFindings).mock.calls[1]?.[0]).toEqual({
        matching: {},
      }),
    );
  });

  it('offers to acknowledge all open findings only while they are listed', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listFindingsPage).mockResolvedValue({ items: [] });
    vi.mocked(countFindings).mockResolvedValue({ count: 7 });
    renderPage('findings', { state: findingStates.resolved });
    expect(await screen.findByText('7 findings')).toBeTruthy();
    expect(
      screen.queryByRole('button', { name: 'Acknowledge all 7 open' }),
    ).toBeNull();
    expect(vi.mocked(countFindings)).toHaveBeenCalledTimes(1);
  });

  it('names the rule revision and scope of each run', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRuleRuns).mockResolvedValue([
      {
        id,
        rule_id: id,
        rule_version: 1,
        definition_hash: '',
        source: 'manual',
        dry_run: false,
        scope_record_id: null,
        status: 'completed',
        candidate_cursor: null,
        candidates_evaluated: 3,
        candidates_skipped: 1,
        findings_created: 1,
        findings_resolved: 0,
        attempts: 1,
        last_error: null,
        truncated: false,
        completed_at: null,
        created_at: '',
      },
      {
        id: '323e4567-e89b-42d3-a456-426614174000',
        rule_id: '423e4567-e89b-42d3-a456-426614174000',
        rule_version: 4,
        definition_hash: '',
        source: 'event',
        dry_run: false,
        scope_record_id: id,
        status: 'completed',
        candidate_cursor: null,
        candidates_evaluated: 1,
        candidates_skipped: 0,
        findings_created: 0,
        findings_resolved: 0,
        attempts: 1,
        last_error: null,
        truncated: false,
        completed_at: null,
        created_at: '',
      },
    ]);
    renderPage('runs');
    await screen.findByText('Check v1');
    const [, manual, scoped] = screen.getAllByRole('row');
    expect(within(manual).getByText('Check v1')).toBeTruthy();
    expect(within(manual).getByText('All records')).toBeTruthy();
    expect(within(manual).getByText('3 (1 skipped)')).toBeTruthy();
    // A revision missing from the definitions still identifies its rule.
    expect(within(scoped).getByText('Rule 423e4567 v4')).toBeTruthy();
    expect(
      within(scoped).getByRole('link', { name: `Record ${id}` }).textContent,
    ).toBe(id.slice(0, 8));
  });

  it('lists the findings that match the filters in the URL', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listFindingsPage).mockResolvedValue({ items: [] });
    renderPage('findings', { severity: 'error', rule: id });
    expect(await screen.findByText('No findings match these filters.'));
    expect(listFindingsPage).toHaveBeenCalledWith(
      { severity: 'error', rule: id },
      0,
    );
  });

  it('keeps a changed filter in the URL', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listFindingsPage).mockResolvedValue({ items: [finding] });
    renderPage('findings', { state: 'resolved' });
    const user = userEvent.setup();
    await user.click(await screen.findByRole('combobox', { name: 'Severity' }));
    await user.click(screen.getByRole('option', { name: 'Critical' }));
    expect(navigate).toHaveBeenCalledWith({
      to: '/manage/rules/findings',
      search: { state: 'resolved', severity: 'critical' },
    });
    await user.click(screen.getByRole('combobox', { name: 'Rule' }));
    await user.click(screen.getByRole('option', { name: 'Check' }));
    expect(navigate).toHaveBeenLastCalledWith({
      to: '/manage/rules/findings',
      search: { state: 'resolved', rule: id },
    });
  });

  it('shows when each run and the latest run executed', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(new Date('2026-10-26T12:00:00Z'));
    vi.mocked(currentSession).mockResolvedValue(manager);
    const run: RuleRun = {
      id,
      rule_id: id,
      rule_version: 1,
      definition_hash: '',
      source: 'schedule',
      dry_run: true,
      scope_record_id: null,
      status: 'completed',
      candidate_cursor: null,
      candidates_evaluated: 2,
      candidates_skipped: 0,
      findings_created: 1,
      findings_resolved: 0,
      attempts: 1,
      last_error: null,
      truncated: false,
      completed_at: '2026-10-24T12:00:00Z',
      created_at: '2026-10-24T11:59:00Z',
    };
    vi.mocked(listRuleRuns).mockResolvedValue([
      run,
      {
        ...run,
        id: crypto.randomUUID(),
        source: 'event',
        dry_run: false,
        completed_at: null,
        created_at: '2026-10-26T09:00:00Z',
      },
    ]);
    renderPage('runs');
    await screen.findAllByText('Check v1');
    const [, scheduled, queued] = screen.getAllByRole('row');
    expect(within(scheduled).getByText('(2 days ago)')).toBeTruthy();
    expect(within(scheduled).getByText('Schedule (dry run)')).toBeTruthy();
    expect(within(queued).getByText('(3 hours ago)')).toBeTruthy();
    expect(within(queued).getByText('Event')).toBeTruthy();
    const summary = screen.getByText(/Last executed/);
    expect(within(summary).getByText('(3 hours ago)')).toBeTruthy();
  });

  it('names blueprints and the default context in their filters', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listFindingsPage).mockResolvedValue({ items: [] });
    vi.mocked(listBlueprints).mockResolvedValueOnce([
      { id, kind: 'record', name: '{{Brand}}' } as Blueprint,
    ]);
    vi.mocked(listContexts).mockResolvedValueOnce([
      { id, code: 'default' } as AttributeContext,
    ]);
    renderPage('findings');
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole('combobox', { name: 'Blueprint' }),
    );
    expect(await screen.findByRole('option', { name: 'Brand' }));
    await user.keyboard('{Escape}');
    await user.click(screen.getByRole('combobox', { name: 'Context' }));
    expect(await screen.findByRole('option', { name: 'Default' }));
  });

  it('loads the next page of findings', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    const second = { ...finding, id: crypto.randomUUID(), message: 'Later' };
    vi.mocked(listFindingsPage)
      .mockResolvedValueOnce({ items: [finding], nextOffset: 1 })
      .mockResolvedValueOnce({ items: [second] });
    renderPage('findings');
    await userEvent
      .setup()
      .click(await screen.findByRole('button', { name: 'Load more' }));
    expect(await screen.findByText('Later')).toBeTruthy();
    expect(screen.getByText('Check failed')).toBeTruthy();
    expect(listFindingsPage).toHaveBeenLastCalledWith({}, 1);
    expect(screen.queryByRole('button', { name: 'Load more' })).toBeNull();
  });
});
