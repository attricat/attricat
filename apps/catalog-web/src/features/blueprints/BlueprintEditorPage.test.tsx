// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { authQueryKeys } from '../auth/queryKeys';
import { draftEditors } from '../drafts/constants';
import { draftStorageKey, writeDraft } from '../drafts/draftStorage';
import { createBlueprint } from './api';
import { BlueprintEditorPage } from './BlueprintEditorPage';

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
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
vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  createBlueprint: vi.fn(),
}));

const session = { user_id: 'user-1', workspace_id: 'workspace-1' };
const draftKey = draftStorageKey({
  editor: draftEditors.blueprintCreate,
  resource: [],
  userId: session.user_id,
  workspaceId: session.workspace_id,
});
const storedDraft = () =>
  JSON.parse(sessionStorage.getItem(draftKey) ?? 'null')?.value;

const renderWithSession = () => {
  const client = new QueryClient({
    defaultOptions: { queries: { staleTime: Infinity } },
  });
  client.setQueryData(authQueryKeys.session(), session);
  render(
    <QueryClientProvider client={client}>
      <BlueprintEditorPage />
    </QueryClientProvider>,
  );
};

// Open modal dialogs hide the editor from the accessibility tree.
const definitionBox = () =>
  screen.getByRole('textbox', { hidden: true, name: 'Definition' });

const editDefinition = (value: string) =>
  fireEvent.change(definitionBox(), {
    target: { value },
  });

afterEach(() => {
  sessionStorage.clear();
  vi.mocked(createBlueprint).mockReset();
});

describe('BlueprintEditorPage', () => {
  it('asks before restoring a draft over the loaded form', async () => {
    writeDraft(draftKey, {
      savedAt: new Date().toISOString(),
      source: null,
      value: 'code = "draft"',
    });
    renderWithSession();

    const dialog = await screen.findByRole('dialog', {
      name: 'Restore unsaved draft?',
    });
    expect(screen.queryByRole('dialog', { name: /example/i })).toBeNull();
    expect(definitionBox()).not.toHaveProperty('value', 'code = "draft"');
    fireEvent.click(
      within(dialog).getByRole('button', { name: 'Restore draft' }),
    );
    expect(definitionBox()).toHaveProperty('value', 'code = "draft"');
  });

  it('keeps the loaded form when a draft is discarded', async () => {
    writeDraft(draftKey, {
      savedAt: new Date().toISOString(),
      source: null,
      value: 'code = "draft"',
    });
    renderWithSession();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Discard draft' }),
    );
    expect(sessionStorage.getItem(draftKey)).toBeNull();
    expect(definitionBox()).not.toHaveProperty('value', 'code = "draft"');
  });

  it('keeps the draft after a failed save and clears it after a successful one', async () => {
    vi.mocked(createBlueprint).mockRejectedValueOnce(new Error('Invalid'));
    renderWithSession();
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    editDefinition('code = "edited"');
    await waitFor(() => expect(storedDraft()).toBe('code = "edited"'));

    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }));
    await screen.findByText(/Invalid/);
    expect(storedDraft()).toBe('code = "edited"');

    vi.mocked(createBlueprint).mockResolvedValueOnce({
      blueprint: { id: 'blueprint-1' },
    } as Awaited<ReturnType<typeof createBlueprint>>);
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }));
    await waitFor(() => expect(sessionStorage.getItem(draftKey)).toBeNull());
  });

  it('locks its editor and template controls while saving a draft', async () => {
    vi.mocked(createBlueprint).mockImplementation(() => new Promise(() => {}));
    render(
      <QueryClientProvider client={new QueryClient()}>
        <BlueprintEditorPage />
      </QueryClientProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    fireEvent.change(screen.getByRole('textbox', { name: 'Definition' }), {
      target: { value: 'code = "saved"' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }));
    await waitFor(() =>
      expect(createBlueprint).toHaveBeenCalledWith('code = "saved"'),
    );
    expect(
      screen
        .getByRole('textbox', { name: 'Definition' })
        .hasAttribute('disabled'),
    ).toBe(true);
    expect(
      screen.getByRole('button', { name: 'Examples' }).hasAttribute('disabled'),
    ).toBe(true);
  });
});
