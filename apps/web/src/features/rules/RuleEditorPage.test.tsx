// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError } from '../../api/request';
import { authQueryKeys } from '../auth/queryKeys';
import {
  getBlueprintRevision,
  listBlueprintRevisions,
  listBlueprints,
  type Blueprint,
  type BlueprintWithAttributes,
} from '../blueprints/api';
import { listContexts } from '../contexts/api';
import {
  createRule,
  getRule,
  getRuleRun,
  runRuleNow,
  updateRule,
  validateRule,
  type CompiledRule,
  type RuleDetails,
  type RuleRunDetails,
} from './api';
import { RuleEditorPage } from './RuleEditorPage';
import { ruleTemplates } from './templates';

const navigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigate,
  createLink: <T,>(component: T) => component,
  Link: ({ children }: { children: React.ReactNode }) => (
    <a href="/records">{children}</a>
  ),
}));
vi.mock('@monaco-editor/react', () => ({
  Editor: ({
    onChange,
    options,
    value,
  }: {
    onChange: (value: string) => void;
    options: { readOnly: boolean };
    value: string;
  }) => (
    <textarea
      aria-label="Definition"
      disabled={options.readOnly}
      onChange={(event) => onChange(event.target.value)}
      value={value}
    />
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
vi.mock('./api', () => ({
  createRule: vi.fn(),
  getRule: vi.fn(),
  getRuleRun: vi.fn(),
  runRuleNow: vi.fn(),
  updateRule: vi.fn(),
  validateRule: vi.fn(),
}));
vi.mock('../blueprints/api', () => ({
  getBlueprintRevision: vi.fn(),
  listBlueprintRevisions: vi.fn(),
  listBlueprints: vi.fn(),
}));
vi.mock('../contexts/api', () => ({ listContexts: vi.fn() }));
vi.mock('../records/api', () => ({
  getRecordLabels: vi.fn(async () => ({
    items: [
      {
        id: '00000000-0000-4000-8000-00000000000a',
        display: { default: 'Blue chair' },
      },
    ],
  })),
}));

const blueprintId = '00000000-0000-4000-8000-000000000002';
const ruleId = '00000000-0000-4000-8000-000000000003';
const blueprintRevision = (version: number, status: string) =>
  ({
    id: blueprintId,
    kind: 'record',
    name: 'Product',
    status,
    version,
  }) as Blueprint;
const compiled: CompiledRule = {
  code: 'title-required',
  name: 'Records have a title',
  severity: 'warning',
  triggers: [{ type: 'manual' }],
  predicate: { type: 'required', attribute_code: 'title' },
};
const rule: RuleDetails = {
  id: ruleId,
  blueprint_id: blueprintId,
  context_id: null,
  code: 'title-required',
  name: 'Records have a title',
  version: 1,
  definition: ruleTemplates[0].definition,
  definition_hash: 'h1',
  compiled_plan: {},
  enabled: true,
  created_at: '',
  updated_at: '',
  not_applicable: [],
};

const renderPage = (props: Parameters<typeof RuleEditorPage>[0] = {}) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity } },
  });
  client.setQueryData(authQueryKeys.session(), {
    user_id: 'user-1',
    workspace_id: 'workspace-1',
  });
  render(
    <QueryClientProvider client={client}>
      <RuleEditorPage {...props} />
    </QueryClientProvider>,
  );
};

const definitionBox = () =>
  screen.getByRole('textbox', { hidden: true, name: 'Definition' });

beforeEach(() => {
  vi.mocked(listBlueprints).mockResolvedValue([blueprintRevision(3, 'draft')]);
  // The newest revision is a draft; rules check published ones.
  vi.mocked(listBlueprintRevisions).mockResolvedValue([
    blueprintRevision(3, 'draft'),
    blueprintRevision(2, 'published'),
    blueprintRevision(1, 'published'),
  ]);
  vi.mocked(getBlueprintRevision).mockResolvedValue({
    attributes: [],
    blueprint: blueprintRevision(2, 'published'),
  } as unknown as BlueprintWithAttributes);
  vi.mocked(listContexts).mockResolvedValue([]);
  vi.mocked(validateRule).mockResolvedValue(compiled);
});
afterEach(() => {
  sessionStorage.clear();
  vi.resetAllMocks();
});

describe('RuleEditorPage', () => {
  it('checks and saves a new rule against the latest published blueprint revision', async () => {
    vi.mocked(createRule).mockResolvedValue(rule);
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(
      screen.getByRole('button', { name: 'Save' }).hasAttribute('disabled'),
    ).toBe(true);
    expect(validateRule).not.toHaveBeenCalled();
    // A rule fits blueprint revisions; there is no revision to choose.
    expect(
      screen.queryByRole('combobox', { name: 'Blueprint revision' }),
    ).toBeNull();

    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Blueprint' }));
    fireEvent.click(
      within(await screen.findByRole('listbox')).getByRole('option', {
        name: 'Product',
      }),
    );
    expect(
      await screen.findByText('Ready to save', {}, { timeout: 2000 }),
    ).toBeTruthy();
    const input = {
      blueprint_id: blueprintId,
      context_id: null,
      definition: ruleTemplates[0].definition,
    };
    expect(validateRule).toHaveBeenCalledWith(input, expect.any(AbortSignal));
    // Completions name the attributes of the latest published revision.
    expect(vi.mocked(getBlueprintRevision).mock.calls[0]?.slice(0, 2)).toEqual([
      blueprintId,
      2,
    ]);
    expect(screen.getByText('Required value on title')).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(createRule).toHaveBeenCalledWith(input));
    await waitFor(() =>
      expect(navigate).toHaveBeenCalledWith({
        params: { ruleId },
        to: '/manage/rules/$ruleId',
      }),
    );
  });

  it('saves a changed definition as the rule’s current revision after showing the change', async () => {
    vi.mocked(getRule).mockResolvedValue(rule);
    vi.mocked(updateRule).mockResolvedValue({ ...rule, version: 2 });
    renderPage({ ruleId });
    expect(
      await screen.findByText('Checks Product · All contexts'),
    ).toBeTruthy();
    expect(definitionBox()).toHaveProperty('value', rule.definition);
    expect(
      screen.getByRole('button', { name: 'Save' }).hasAttribute('disabled'),
    ).toBe(true);

    const edited = rule.definition.replace('warning', 'error');
    fireEvent.change(definitionBox(), { target: { value: edited } });
    expect((await screen.findByTestId('diff')).textContent).toBe(
      `${rule.definition} → ${edited}`,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(updateRule).toHaveBeenCalledWith(ruleId, {
        definition: edited,
        accept_existing_violations: undefined,
      }),
    );
    await waitFor(() =>
      expect(navigate).toHaveBeenCalledWith({
        params: { ruleId },
        to: '/manage/rules/$ruleId',
      }),
    );
  });

  it('dry-runs an enforcing change and saves it despite existing violations', async () => {
    vi.mocked(getRule).mockResolvedValue(rule);
    const edited = `${rule.definition}\n[enforcement]\non_save = true\n`;
    const run: RuleRunDetails = {
      id: '00000000-0000-4000-8000-000000000009',
      rule_id: ruleId,
      rule_version: 1,
      definition_hash: 'h2',
      source: 'manual',
      dry_run: true,
      scope_record_id: null,
      status: 'completed',
      candidate_cursor: null,
      candidates_evaluated: 4,
      candidates_skipped: 0,
      findings_created: 2,
      findings_resolved: 0,
      attempts: 1,
      last_error: null,
      truncated: false,
      completed_at: '2026-10-27T10:00:00Z',
      created_at: '2026-10-27T10:00:00Z',
      dry_run_failures: [
        {
          record_id: '00000000-0000-4000-8000-00000000000a',
          message: "'title' is required",
          evidence: {},
        },
      ],
    };
    vi.mocked(runRuleNow).mockResolvedValue({ id: run.id });
    vi.mocked(getRuleRun).mockResolvedValue(run);
    vi.mocked(updateRule)
      .mockRejectedValueOnce(
        new ApiRequestError(
          409,
          'the latest dry run found 2 existing violations',
          'rule_has_existing_violations',
          { existing_violations: 2 },
        ),
      )
      .mockResolvedValueOnce({ ...rule, version: 2 });
    renderPage({ ruleId });
    await screen.findByText('Checks Product · All contexts');
    fireEvent.change(definitionBox(), { target: { value: edited } });
    await screen.findByText('Ready to save', {}, { timeout: 2000 });

    fireEvent.click(
      await screen.findByRole('button', { name: 'Dry run this change' }),
    );
    await waitFor(() =>
      expect(runRuleNow).toHaveBeenCalledWith(ruleId, true, edited),
    );
    expect(
      await screen.findByText('The dry run checked 4 records; 2 would fail.'),
    ).toBeTruthy();
    // It names the records it would flag and what they would report.
    expect(
      await screen.findByRole('link', { name: 'Blue chair' }),
    ).toBeTruthy();
    expect(screen.getByText("'title' is required")).toBeTruthy();
    expect(screen.getByText('Showing the first 1 of 2.')).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    const alert = await screen.findByText(
      'Records already violate this rule (2).',
      { exact: false },
    );
    fireEvent.click(
      within(alert.closest('[role="alert"]') as HTMLElement).getByRole(
        'button',
        { name: 'Save anyway' },
      ),
    );
    await waitFor(() =>
      expect(updateRule).toHaveBeenLastCalledWith(ruleId, {
        definition: edited,
        accept_existing_violations: true,
      }),
    );
  });

  it('says when the rule’s blueprint cannot be loaded', async () => {
    vi.mocked(getRule).mockResolvedValue(rule);
    vi.mocked(getBlueprintRevision).mockRejectedValue(
      new ApiRequestError(404, 'Not found', 'not_found'),
    );
    renderPage({ ruleId });
    expect(
      await screen.findByText("It doesn't exist, or you can't access it."),
    ).toBeTruthy();
  });
});
