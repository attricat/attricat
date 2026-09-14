// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import { validateWorkflow } from './api';
import { WorkflowEditorPage } from './WorkflowEditorPage';

vi.mock('@monaco-editor/react', () => ({
  Editor: ({
    onChange,
    options,
    value,
  }: {
    onChange: (value: string) => void;
    options: { ariaLabel: string };
    value: string;
  }) => (
    <textarea
      aria-label={options.ariaLabel}
      onChange={(event) => onChange(event.target.value)}
      value={value}
    />
  ),
}));
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
}));
vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./api', () => ({
  createWorkflow: vi.fn(),
  createWorkflowRevision: vi.fn(),
  getWorkflowRevision: vi.fn(),
  validateWorkflow: vi.fn(),
}));

describe('WorkflowEditorPage', () => {
  afterEach(() => vi.clearAllMocks());

  it('clears successful validation diagnostics when the definition changes', async () => {
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { workflows_manage: true },
    } as Awaited<ReturnType<typeof currentSession>>);
    vi.mocked(validateWorkflow).mockResolvedValue({
      actions: [],
      code: 'example-workflow',
      format_version: 1,
      name: 'Example workflow',
      raw_definition_hash: 'a'.repeat(64),
      triggers: [],
    });
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const user = userEvent.setup();

    render(
      <QueryClientProvider client={client}>
        <WorkflowEditorPage />
      </QueryClientProvider>,
    );

    await user.click(
      await screen.findByRole('button', { name: 'Validate TOML' }),
    );
    expect(
      await screen.findByText(
        'Valid: Example workflow has 0 triggers and 0 actions.',
      ),
    ).toBeDefined();

    await user.type(
      screen.getByRole('textbox', { name: 'Workflow TOML definition' }),
      '\n# changed',
    );
    expect(
      screen.queryByText(
        'Valid: Example workflow has 0 triggers and 0 actions.',
      ),
    ).toBeNull();
  });
});
