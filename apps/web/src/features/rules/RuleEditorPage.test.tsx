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
  createRuleRevision,
  listRules,
  validateRule,
  type CompiledRule,
  type Rule,
} from './api';
import { RuleEditorPage } from './RuleEditorPage';
import { ruleTemplates } from './templates';

const navigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigate,
  createLink: <T,>(component: T) => component,
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
}));
vi.mock('./api', () => ({
  createRule: vi.fn(),
  createRuleRevision: vi.fn(),
  listRules: vi.fn(),
  validateRule: vi.fn(),
}));
vi.mock('../blueprints/api', () => ({
  getBlueprintRevision: vi.fn(),
  listBlueprintRevisions: vi.fn(),
  listBlueprints: vi.fn(),
}));
vi.mock('../contexts/api', () => ({ listContexts: vi.fn() }));

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
const rule = {
  id: ruleId,
  blueprint_id: blueprintId,
  blueprint_version: 2,
  context_id: null,
  code: 'title-required',
  name: 'Records have a title',
  version: 1,
  status: 'published',
  definition: ruleTemplates[0].definition,
} as Rule;

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
  it('checks and saves a new rule for the latest published blueprint revision', async () => {
    vi.mocked(createRule).mockResolvedValue(rule);
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(
      screen
        .getByRole('button', { name: 'Save draft' })
        .hasAttribute('disabled'),
    ).toBe(true);
    expect(validateRule).not.toHaveBeenCalled();

    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Blueprint' }));
    fireEvent.click(
      within(await screen.findByRole('listbox')).getByRole('option', {
        name: 'Product',
      }),
    );
    await waitFor(() =>
      expect(
        screen.getByRole('combobox', { name: 'Blueprint revision' }),
      ).toHaveProperty('textContent', 'Version 2'),
    );
    expect(
      await screen.findByText('Ready to save', {}, { timeout: 2000 }),
    ).toBeTruthy();
    const scope = {
      blueprint_id: blueprintId,
      blueprint_version: 2,
      context_id: null,
      definition: ruleTemplates[0].definition,
    };
    expect(validateRule).toHaveBeenCalledWith(scope, expect.any(AbortSignal));
    expect(screen.getByText('Required value on title')).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }));
    await waitFor(() => expect(createRule).toHaveBeenCalledWith(scope));
    await waitFor(() =>
      expect(navigate).toHaveBeenCalledWith({
        params: { ruleId, version: '1' },
        to: '/manage/rules/$ruleId/revisions/$version',
      }),
    );
  });

  it('saves a changed revision with the rule’s blueprint revision and context', async () => {
    vi.mocked(listRules).mockResolvedValue([rule]);
    vi.mocked(createRuleRevision).mockResolvedValue({ ...rule, version: 2 });
    renderPage({ ruleId, sourceVersion: 1 });
    expect(
      await screen.findByText('Checks Product v2 · All contexts'),
    ).toBeTruthy();
    expect(definitionBox()).toHaveProperty('value', rule.definition);
    expect(
      screen
        .getByRole('button', { name: 'Save draft' })
        .hasAttribute('disabled'),
    ).toBe(true);

    const edited = rule.definition.replace('warning', 'error');
    fireEvent.change(definitionBox(), { target: { value: edited } });
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }));
    await waitFor(() =>
      expect(createRuleRevision).toHaveBeenCalledWith(ruleId, {
        blueprint_id: blueprintId,
        blueprint_version: 2,
        context_id: null,
        definition: edited,
      }),
    );
  });

  it('says when the rule’s blueprint revision cannot be loaded', async () => {
    vi.mocked(listRules).mockResolvedValue([rule]);
    vi.mocked(getBlueprintRevision).mockRejectedValue(
      new ApiRequestError(404, 'Not found', 'not_found'),
    );
    renderPage({ ruleId, sourceVersion: 1 });
    expect(
      await screen.findByText("It doesn't exist, or you can't access it."),
    ).toBeTruthy();
  });
});
