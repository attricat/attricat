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
import { draftEditors } from '../drafts/constants';
import { draftStorageKey, writeDraft } from '../drafts/draftStorage';
import {
  createBlueprint,
  validateBlueprint,
  type BlueprintWithAttributes,
} from './api';
import { blueprintTemplates } from './blueprintEditorUtils';
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
  listBlueprints: vi.fn(async () => []),
  validateBlueprint: vi.fn(),
}));

const checkedBlueprint = {
  attributes: [
    {
      code: 'price',
      name: 'Price',
      value_type: 'number',
      value_schema: { minimum: 0, type: 'number' },
    },
  ],
  blueprint: {
    record_schema: null,
    views: {
      detail: { children: [{ field: 'price', type: 'field' }], type: 'stack' },
      dropdown_option: { fields: ['price'], type: 'dropdown_option' },
    },
  },
} as unknown as BlueprintWithAttributes;

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

beforeEach(() => {
  vi.mocked(validateBlueprint).mockResolvedValue(checkedBlueprint);
});
afterEach(() => {
  sessionStorage.clear();
  vi.mocked(createBlueprint).mockReset();
  vi.mocked(validateBlueprint).mockReset();
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
    fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
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
    fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
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

  it('previews the checked views and checks sandbox values', async () => {
    renderWithSession();
    fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(
      await screen.findByText('Ready to save', {}, { timeout: 2000 }),
    ).toBeTruthy();
    expect(validateBlueprint).toHaveBeenCalledWith(
      blueprintTemplates[0].definition,
      undefined,
      expect.any(AbortSignal),
    );
    expect(screen.getByRole('tab', { name: 'detail' })).toBeTruthy();
    const price = screen.getByRole('spinbutton', { name: 'Price' });
    fireEvent.change(price, { target: { value: '-1' } });
    expect(price.getAttribute('aria-invalid')).toBe('true');
  });

  it('points at the line a failed check names', async () => {
    renderWithSession();
    fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
    await screen.findByText('Ready to save', {}, { timeout: 2000 });
    vi.mocked(validateBlueprint).mockRejectedValue(
      new ApiRequestError(
        422,
        "view 'dropdown_option' references unknown attribute 'sku'",
        'invalid_blueprint_definition',
      ),
    );
    editDefinition(
      'format_version: 1\ncode: p\nviews:\n  dropdown_option:\n    type: dropdown_option\n    fields: [sku]\n',
    );
    expect(
      await screen.findByText('Problem on line 6', {}, { timeout: 2000 }),
    ).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Go to line 6' })).toBeTruthy();
  });
});
