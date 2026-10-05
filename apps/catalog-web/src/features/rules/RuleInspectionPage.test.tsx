// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { forwardRef, type ComponentPropsWithoutRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import { currentSession } from '../auth/api';
import {
  acknowledgeFinding,
  disableRule,
  enableRuleRevision,
  listFindings,
  listRuleRuns,
  listRules,
  runRuleNow,
  type Finding,
  type Rule,
} from './api';
import { RuleInspectionPage } from './RuleInspectionPage';

vi.mock('@tanstack/react-router', () => ({
  Link: ({
    children,
    params,
    to,
    ...props
  }: ComponentPropsWithoutRef<'a'> & {
    params: { entityId: string };
    to: string;
  }) => (
    <a {...props} href={to.replace('$entityId', params.entityId)}>
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
vi.mock('./api', () => ({
  acknowledgeFinding: vi.fn(),
  disableRule: vi.fn(),
  enableRuleRevision: vi.fn(),
  listFindings: vi.fn(),
  listRuleRuns: vi.fn(),
  listRules: vi.fn(),
  runRuleNow: vi.fn(),
}));

const id = '123e4567-e89b-42d3-a456-426614174000';
const manager = {
  capabilities: { rules_read: true, rules_manage: true },
} as Awaited<ReturnType<typeof currentSession>>;
const rule: Rule = {
  id,
  blueprint_id: id,
  blueprint_version: 1,
  context_id: null,
  code: 'check',
  name: 'Check',
  version: 1,
  status: 'published',
  definition: '',
  definition_hash: '',
  compiled_plan: {},
  published_at: null,
  created_at: '',
  enabled_version: null,
};
const finding: Finding = {
  id,
  rule_id: id,
  rule_version: 1,
  entity_id: id,
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
const renderPage = (section: 'rules' | 'findings' | 'runs') => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <RuleInspectionPage section={section} />
    </QueryClientProvider>,
  );
};

describe('RuleInspectionPage', () => {
  beforeEach(() => vi.mocked(listRules).mockResolvedValue([rule]));
  afterEach(() => vi.clearAllMocks());

  it('disables both run variants while a run is pending', async () => {
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { rules_read: true, rules_manage: true },
    } as Awaited<ReturnType<typeof currentSession>>);
    vi.mocked(listRules).mockResolvedValue([
      {
        id,
        blueprint_id: id,
        blueprint_version: 1,
        context_id: null,
        code: 'check',
        name: 'Check',
        version: 1,
        status: 'published',
        definition: '',
        definition_hash: '',
        compiled_plan: {},
        published_at: null,
        created_at: '',
        enabled_version: 1,
      },
    ]);
    vi.mocked(runRuleNow).mockImplementation(() => new Promise(() => {}));
    renderPage('rules');
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Dry run' }));
    expect(runRuleNow).toHaveBeenCalledWith(id, 1, true);
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
    vi.mocked(listFindings).mockResolvedValue([
      {
        id,
        rule_id: id,
        rule_version: 1,
        entity_id: id,
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
    ]);
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

  it('shows why a rule run was refused', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRules).mockResolvedValue([rule]);
    vi.mocked(runRuleNow).mockRejectedValue(
      new Error('rule has no enabled revision'),
    );
    renderPage('rules');
    await userEvent
      .setup()
      .click(await screen.findByRole('button', { name: 'Run now' }));
    expect((await screen.findByRole('alert')).textContent).toBe(
      'rule has no enabled revision',
    );
  });

  it('shows why an acknowledgement was refused', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listFindings).mockResolvedValue([finding]);
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

  it('enables a published revision from its switch', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRules).mockResolvedValue([rule]);
    vi.mocked(enableRuleRevision).mockResolvedValue({
      ...rule,
      enabled_version: 1,
    });
    renderPage('rules');
    await userEvent
      .setup()
      .click(await screen.findByRole('switch', { name: 'Enable Check v1' }));
    expect(enableRuleRevision).toHaveBeenCalledWith(id, 1, undefined);
  });

  it('marks only the enabled revision and disables the rule', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRules).mockResolvedValue([
      { ...rule, version: 2, enabled_version: 2 },
      { ...rule, version: 1, enabled_version: 2 },
      { ...rule, version: 3, status: 'draft', enabled_version: 2 },
    ]);
    vi.mocked(disableRule).mockResolvedValue({
      ...rule,
      enabled_version: null,
    });
    renderPage('rules');
    const enabled = await screen.findByRole('switch', {
      name: 'Enable Check v2',
    });
    const older = screen.getByRole('switch', { name: 'Enable Check v1' });
    expect((enabled as HTMLInputElement).checked).toBe(true);
    expect((older as HTMLInputElement).checked).toBe(false);
    const [, v1Row, draftRow] = screen
      .getAllByRole('row')
      .filter((row) => row.textContent?.includes('Check'));
    expect(within(v1Row).getByText('Disabled')).toBeTruthy();
    expect(within(draftRow).queryByRole('switch')).toBeNull();
    expect(within(draftRow).getByText('Draft')).toBeTruthy();

    await userEvent.setup().click(enabled);
    expect(disableRule).toHaveBeenCalledWith(id);
  });

  it('offers to accept the existing violations of an enforcing rule', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRules).mockResolvedValue([rule]);
    vi.mocked(enableRuleRevision)
      .mockRejectedValueOnce(
        new ApiRequestError(
          409,
          'the latest dry run found 3 existing violations',
          'rule_has_existing_violations',
          { existing_violations: 3 },
        ),
      )
      .mockResolvedValueOnce({ ...rule, enabled_version: 1 });
    renderPage('rules');
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole('switch', { name: 'Enable Check v1' }),
    );
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain(
      'the latest dry run found 3 existing violations',
    );
    await user.click(
      within(alert).getByRole('button', { name: 'Enable anyway' }),
    );
    expect(enableRuleRevision).toHaveBeenLastCalledWith(id, 1, true);
  });

  it('names the rule revision and links the entity of each finding', async () => {
    const entityId = '223e4567-e89b-42d3-a456-426614174000';
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listFindings).mockResolvedValue([
      { ...finding, entity_id: entityId },
    ]);
    renderPage('findings');
    const row = (await screen.findByText('Check failed')).closest('tr')!;
    expect(await within(row).findByText('Check v1')).toBeTruthy();
    const link = within(row).getByRole('link', {
      name: `Open entity ${entityId}`,
    });
    expect(link.getAttribute('href')).toBe(`/entities/${entityId}`);
    expect(link.textContent).toBe('223e4567');
  });

  it('names the rule revision and scope of each run', async () => {
    vi.mocked(currentSession).mockResolvedValue(manager);
    vi.mocked(listRuleRuns).mockResolvedValue([
      {
        id,
        rule_id: id,
        rule_version: 1,
        source: 'manual',
        dry_run: false,
        scope_entity_id: null,
        status: 'completed',
        candidate_cursor: null,
        candidates_evaluated: 2,
        findings_created: 1,
        findings_resolved: 0,
        attempts: 1,
        last_error: null,
        completed_at: null,
        created_at: '',
      },
      {
        id: '323e4567-e89b-42d3-a456-426614174000',
        rule_id: '423e4567-e89b-42d3-a456-426614174000',
        rule_version: 4,
        source: 'event',
        dry_run: false,
        scope_entity_id: id,
        status: 'completed',
        candidate_cursor: null,
        candidates_evaluated: 1,
        findings_created: 0,
        findings_resolved: 0,
        attempts: 1,
        last_error: null,
        completed_at: null,
        created_at: '',
      },
    ]);
    renderPage('runs');
    await screen.findByText('Check v1');
    const [, manual, scoped] = screen.getAllByRole('row');
    expect(within(manual).getByText('Check v1')).toBeTruthy();
    expect(within(manual).getByText('All entities')).toBeTruthy();
    // A revision missing from the definitions still identifies its rule.
    expect(within(scoped).getByText('Rule 423e4567 v4')).toBeTruthy();
    expect(
      within(scoped).getByRole('link', { name: `Open entity ${id}` }),
    ).toBeTruthy();
  });
});
