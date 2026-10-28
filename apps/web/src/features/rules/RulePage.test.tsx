// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { forwardRef, type ComponentPropsWithoutRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import { listBlueprints, type Blueprint } from '../blueprints/api';
import { listContexts } from '../contexts/api';
import { getDirectory } from '../principals/api';
import {
  disableRule,
  getRule,
  getRuleRun,
  listRecentRuleRuns,
  listRuleRevisions,
  runRuleNow,
  type RuleDetails,
  type RuleRevision,
} from './api';
import { RULE_PAGE_CHANGELOG, RULE_PAGE_DEFINITION } from './constants';
import { RulePage } from './RulePage';

const navigate = vi.fn();
type LinkProps = ComponentPropsWithoutRef<'a'> & {
  params?: Record<string, string>;
  search?: Record<string, number | string>;
  to?: string;
};
vi.mock('@tanstack/react-router', () => {
  /** Renders a router link as an anchor whose href shows its target. */
  const href = ({ params, search, to }: LinkProps) => {
    const path = Object.entries(params ?? {}).reduce(
      (target, [key, value]) => target.replace(`$${key}`, value),
      to ?? '',
    );
    const query = new URLSearchParams(
      Object.entries(search ?? {}).map(([key, value]) => [key, String(value)]),
    ).toString();
    return query ? `${path}?${query}` : path;
  };
  const Anchor = forwardRef<HTMLAnchorElement, LinkProps>(
    ({ children, params, search, to, ...props }, ref) => (
      <a {...props} href={href({ params, search, to })} ref={ref}>
        {children}
      </a>
    ),
  );
  return {
    useNavigate: () => navigate,
    Link: Anchor,
    createLink: () => Anchor,
  };
});
vi.mock('@monaco-editor/react', () => ({
  Editor: ({ value }: { value: string }) => (
    <textarea aria-label="Definition" readOnly value={value} />
  ),
  DiffEditor: ({
    modified,
    original,
  }: {
    modified: string;
    original: string;
  }) => (
    <div data-testid="diff">
      {original} → {modified}
    </div>
  ),
}));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('../blueprints/api', () => ({
  listBlueprintRevisions: vi.fn(() => Promise.resolve([])),
  listBlueprints: vi.fn(),
}));
vi.mock('../contexts/api', () => ({ listContexts: vi.fn() }));
vi.mock('../principals/api', () => ({ getDirectory: vi.fn() }));
vi.mock('./api', () => ({
  disableRule: vi.fn(),
  enableRule: vi.fn(),
  getRule: vi.fn(),
  getRuleRun: vi.fn(),
  listRecentRuleRuns: vi.fn(),
  listRuleRevisions: vi.fn(),
  runRuleNow: vi.fn(),
}));

const ruleId = '00000000-0000-4000-8000-000000000003';
const blueprintId = '00000000-0000-4000-8000-000000000002';
const authorId = '00000000-0000-4000-8000-000000000004';
const rule: RuleDetails = {
  id: ruleId,
  blueprint_id: blueprintId,
  context_id: null,
  code: 'title-required',
  name: 'Records have a title',
  version: 3,
  definition: 'severity = "critical"',
  definition_hash: 'h3',
  compiled_plan: {},
  enabled: true,
  created_at: '2026-10-20T10:00:00Z',
  updated_at: '2026-10-26T10:00:00Z',
  not_applicable: [
    {
      blueprint_version: 2,
      reason: 'attribute_missing',
      message: "unknown attribute 'title'",
    },
  ],
};
const revision = (version: number, severity: string): RuleRevision => ({
  rule_id: ruleId,
  version,
  name: rule.name,
  definition: `severity = "${severity}"`,
  definition_hash: `h${version}`,
  created_by_user_id: version === 1 ? null : authorId,
  created_at: `2026-10-2${version}T10:00:00Z`,
});

const renderPage = (props: Partial<Parameters<typeof RulePage>[0]>) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <RulePage ruleId={ruleId} section={RULE_PAGE_DEFINITION} {...props} />
    </QueryClientProvider>,
  );
};

beforeEach(() => {
  vi.mocked(currentSession).mockResolvedValue({
    capabilities: { rules_read: true, rules_manage: true },
  } as Awaited<ReturnType<typeof currentSession>>);
  vi.mocked(getRule).mockResolvedValue(rule);
  vi.mocked(listBlueprints).mockResolvedValue([
    {
      id: blueprintId,
      kind: 'record',
      name: '{{Product}}',
      version: 3,
    } as Blueprint,
  ]);
  vi.mocked(listContexts).mockResolvedValue([]);
  vi.mocked(listRecentRuleRuns).mockResolvedValue([]);
  vi.mocked(listRuleRevisions).mockResolvedValue([
    revision(3, 'critical'),
    revision(2, 'error'),
    revision(1, 'warning'),
  ]);
  vi.mocked(getDirectory).mockResolvedValue({
    users: [
      {
        id: authorId,
        display_name: 'Ada',
        email: 'ada@example.test',
        active: true,
      },
    ],
    teams: [],
  });
});
afterEach(() => vi.resetAllMocks());

describe('RulePage', () => {
  it('shows what the rule checks, the revisions it skips, and disables it', async () => {
    vi.mocked(disableRule).mockResolvedValue({ ...rule, enabled: false });
    renderPage({});
    expect(
      await screen.findByRole('heading', { name: 'Records have a title' }),
    ).toBeTruthy();
    expect(await screen.findByRole('link', { name: 'Product' })).toBeTruthy();
    expect(
      screen.getByText(
        "Revision 2: an attribute the rule names is missing (unknown attribute 'title')",
      ),
    ).toBeTruthy();
    expect(screen.getByRole('textbox', { name: 'Definition' })).toHaveProperty(
      'value',
      rule.definition,
    );
    expect(
      screen.getByRole('link', { name: 'Edit' }).getAttribute('href'),
    ).toBe(`/manage/rules/${ruleId}/edit`);

    await userEvent
      .setup()
      .click(screen.getByRole('button', { name: 'Disable' }));
    await waitFor(() => expect(disableRule).toHaveBeenCalledWith(ruleId));
  });

  it('counts its findings, lists its recent runs and dry-runs it', async () => {
    vi.mocked(getRule).mockResolvedValue({
      ...rule,
      compiled_plan: {
        severity: 'critical',
        description: 'Every product needs a title to be found.',
      },
      open_findings: 4,
      acknowledged_findings: 1,
    });
    vi.mocked(runRuleNow).mockResolvedValue({ id: ruleId });
    vi.mocked(getRuleRun).mockReturnValue(new Promise(() => undefined));
    renderPage({});
    expect(
      (await screen.findByRole('link', { name: '4 open' })).getAttribute(
        'href',
      ),
    ).toBe(`/manage/rules/findings?rule=${ruleId}&state=open`);
    expect(screen.getByRole('link', { name: '1 acknowledged' })).toBeTruthy();
    expect(screen.getByText('Critical')).toBeTruthy();
    expect(
      screen.getByText('Every product needs a title to be found.'),
    ).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Recent runs' })).toBeTruthy();
    expect(
      screen
        .getByRole('link', { name: 'All runs of this rule' })
        .getAttribute('href'),
    ).toBe(`/manage/rules/runs?rule=${ruleId}`);

    await userEvent
      .setup()
      .click(screen.getByRole('button', { name: 'Dry run' }));
    await waitFor(() => expect(runRuleNow).toHaveBeenCalledWith(ruleId, true));
    expect(
      await screen.findByRole('dialog', {
        name: 'Dry run of Records have a title',
      }),
    ).toBeTruthy();
  });

  it('lists the changelog and compares a revision with the one before it', async () => {
    renderPage({ section: RULE_PAGE_CHANGELOG, comparison: {} });
    const rows = await screen.findAllByRole('row');
    const [, current, second, first] = rows;
    expect(within(current).getByText('Current')).toBeTruthy();
    expect(await within(second).findByText('Ada')).toBeTruthy();
    expect(within(first).getByText('Not recorded')).toBeTruthy();
    expect(
      within(second)
        .getByRole('link', { name: 'Compare with previous' })
        .getAttribute('href'),
    ).toBe(`/manage/rules/${ruleId}/changelog?from=1&to=2`);
    // The first revision has nothing before it.
    expect(within(first).queryByRole('link')).toBeNull();
    // The current revision is compared with the previous one by default.
    expect((await screen.findByTestId('diff')).textContent).toBe(
      'severity = "error" → severity = "critical"',
    );
  });

  it('compares the revisions the URL names', async () => {
    renderPage({
      section: RULE_PAGE_CHANGELOG,
      comparison: { from: 1, to: 3 },
    });
    expect((await screen.findByTestId('diff')).textContent).toBe(
      'severity = "warning" → severity = "critical"',
    );
    await userEvent
      .setup()
      .click(screen.getByRole('combobox', { name: 'From' }));
    await userEvent
      .setup()
      .click(await screen.findByRole('option', { name: 'v2' }));
    expect(navigate).toHaveBeenCalledWith({
      params: { ruleId },
      replace: true,
      search: { from: 2, to: 3 },
      to: '/manage/rules/$ruleId/changelog',
    });
  });

  it('shows the first revision on its own', async () => {
    renderPage({ section: RULE_PAGE_CHANGELOG, comparison: { to: 1 } });
    expect(
      await screen.findByText("This is the rule's first revision."),
    ).toBeTruthy();
    expect(screen.queryByTestId('diff')).toBeNull();
  });
});
