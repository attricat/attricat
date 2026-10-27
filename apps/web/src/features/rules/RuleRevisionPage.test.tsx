// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ComponentPropsWithoutRef } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import { getBlueprintRevision } from '../blueprints/api';
import { listRules, publishRuleRevision, type Rule } from './api';
import { RuleRevisionPage } from './RuleRevisionPage';

vi.mock('@tanstack/react-router', () => ({
  createLink: <T,>(component: T) => component,
  Link: ({
    children,
    params,
    to,
    ...props
  }: ComponentPropsWithoutRef<'a'> & {
    params?: Record<string, string>;
    to: string;
  }) => (
    <a
      {...props}
      href={Object.entries(params ?? {}).reduce(
        (href, [key, value]) => href.replace(`$${key}`, value),
        to,
      )}
    >
      {children}
    </a>
  ),
}));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('../blueprints/api', () => ({ getBlueprintRevision: vi.fn() }));
vi.mock('./api', () => ({
  listRules: vi.fn(),
  publishRuleRevision: vi.fn(),
}));
vi.mock('@monaco-editor/react', () => ({
  Editor: ({
    options,
    value,
  }: {
    options: { ariaLabel: string; readOnly: boolean };
    value: string;
  }) => (
    <textarea
      aria-label={options.ariaLabel}
      readOnly={options.readOnly}
      value={value}
    />
  ),
}));

const ruleId = '00000000-0000-4000-8000-000000000003';
const blueprintId = '00000000-0000-4000-8000-000000000002';
const definition = `code = "title_required"
[predicate]
type = "required"
attribute_code = "title"
`;
const rule: Rule = {
  id: ruleId,
  blueprint_id: blueprintId,
  blueprint_version: 2,
  context_id: null,
  code: 'title_required',
  name: 'Title required',
  version: 1,
  status: 'published',
  definition,
  definition_hash: '',
  compiled_plan: {},
  published_at: null,
  created_at: '',
  enabled_version: 1,
};

const renderPage = (version: number | undefined) =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <RuleRevisionPage ruleId={ruleId} version={version} />
    </QueryClientProvider>,
  );

describe('RuleRevisionPage', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { rules_read: true },
    } as Awaited<ReturnType<typeof currentSession>>);
    vi.mocked(listRules).mockResolvedValue([rule]);
    vi.mocked(getBlueprintRevision).mockResolvedValue({
      blueprint: { name: 'Product' },
    } as Awaited<ReturnType<typeof getBlueprintRevision>>);
  });

  it('shows the revision’s definition and the blueprint it checks', async () => {
    renderPage(1);
    expect(
      await screen.findByRole('heading', { name: /Title required v1/ }),
    ).toBeTruthy();
    expect(
      (screen.getByLabelText('Definition') as HTMLTextAreaElement).value,
    ).toBe(definition);
    // Reading rules does not offer to change them.
    expect(screen.queryByText('New revision')).toBeNull();
    expect(screen.getByText('Enabled')).toBeTruthy();
    expect(
      (await screen.findByRole('link', { name: 'Product v2' })).getAttribute(
        'href',
      ),
    ).toBe(`/manage/blueprints/${blueprintId}`);
  });

  it('publishes a draft revision for rule managers', async () => {
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { rules_read: true, rules_manage: true },
    } as Awaited<ReturnType<typeof currentSession>>);
    const draft = { ...rule, status: 'draft', enabled_version: null } as const;
    vi.mocked(listRules).mockResolvedValue([draft]);
    vi.mocked(publishRuleRevision).mockResolvedValue({
      ...draft,
      status: 'published',
    });
    renderPage(1);
    expect((await screen.findByText('New revision')).getAttribute('to')).toBe(
      '/manage/rules/$ruleId/revisions/$version/new',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Publish' }));
    await waitFor(() =>
      expect(publishRuleRevision).toHaveBeenCalledWith(ruleId, 1),
    );
  });

  it.each([7, undefined])(
    'says when the revision does not exist (version %s)',
    async (version) => {
      renderPage(version);
      expect(
        await screen.findByText('This rule revision was not found.'),
      ).toBeTruthy();
    },
  );
});
