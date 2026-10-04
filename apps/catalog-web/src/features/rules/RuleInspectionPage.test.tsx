// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { forwardRef, type ComponentPropsWithoutRef } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import {
  acknowledgeFinding,
  listFindings,
  listRules,
  runRuleNow,
  type Finding,
  type Rule,
} from './api';
import { RuleInspectionPage } from './RuleInspectionPage';

vi.mock('@tanstack/react-router', () => ({
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
const renderPage = (section: 'rules' | 'findings') => {
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
});
