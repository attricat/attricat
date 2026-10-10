// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import { createWorkflow, getWorkflowRevision, validateWorkflow } from './api';
import { workflowQueryKeys } from './queryKeys';
import { WorkflowEditorPage } from './WorkflowEditorPage';

vi.mock('@monaco-editor/react', () => ({
  Editor: ({
    onChange,
    options,
    value,
  }: {
    onChange: (value: string) => void;
    options: { ariaLabel: string; readOnly: boolean };
    value: string;
  }) => (
    <textarea
      aria-label={options.ariaLabel}
      onChange={(event) => onChange(event.target.value)}
      readOnly={options.readOnly}
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

  it('locks the definition and validation while saving a submitted draft', async () => {
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { workflows_manage: true },
    } as Awaited<ReturnType<typeof currentSession>>);
    vi.mocked(createWorkflow).mockImplementation(() => new Promise(() => {}));
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={client}>
        <WorkflowEditorPage />
      </QueryClientProvider>,
    );
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Save draft' }));
    expect(createWorkflow).toHaveBeenCalledTimes(1);
    expect(
      (
        screen.getByRole('textbox', {
          name: 'Workflow TOML definition',
        }) as HTMLTextAreaElement
      ).readOnly,
    ).toBe(true);
    expect(
      (
        screen.getByRole('button', {
          name: 'Validate TOML',
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      (screen.getByRole('button', { name: 'Saving…' }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it('does not replace an edited revision with a background refetch', async () => {
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { workflows_manage: true },
    } as Awaited<ReturnType<typeof currentSession>>);
    const source = { definition: 'original TOML' } as Awaited<
      ReturnType<typeof getWorkflowRevision>
    >;
    vi.mocked(getWorkflowRevision)
      .mockResolvedValueOnce(source)
      .mockResolvedValueOnce({
        ...source,
        definition: 'server updated TOML',
      });
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={client}>
        <WorkflowEditorPage workflowId="workflow-1" sourceVersion={1} />
      </QueryClientProvider>,
    );
    const user = userEvent.setup();
    const editor = await screen.findByRole('textbox', {
      name: 'Workflow TOML definition',
    });
    await user.type(editor, ' changed');
    expect((editor as HTMLTextAreaElement).value).toBe('original TOML changed');
    await client.refetchQueries({
      queryKey: workflowQueryKeys.revision('workflow-1', 1),
    });
    expect((editor as HTMLTextAreaElement).value).toBe('original TOML changed');
    expect(getWorkflowRevision).toHaveBeenCalledTimes(2);
  });

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
