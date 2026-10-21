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
import { ApiRequestError } from '../../api/request';
import {
  createReusableAttribute,
  listReusableAttributes,
  validateReusableAttribute,
  type ReusableAttribute,
} from './api';
import '../../i18n';
import { ReusableAttributeEditorPage } from './ReusableAttributeEditorPage';
import { reusableAttributeTemplates } from './templates';

const { navigate } = vi.hoisted(() => ({ navigate: vi.fn() }));
vi.mock('@tanstack/react-router', () => ({
  createLink:
    () =>
    ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  useNavigate: () => navigate,
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
  createReusableAttribute: vi.fn(),
  listReusableAttributes: vi.fn(),
  validateReusableAttribute: vi.fn(),
}));

const checked = (
  overrides: Partial<ReusableAttribute> = {},
): ReusableAttribute => ({
  cardinality: null,
  code: 'size',
  context_editable: 'all',
  context_fallback: 'default',
  default_value: null,
  definition: '',
  definition_id: '123e4567-e89b-12d3-a456-426614174000',
  facetable: true,
  file_policy: null,
  id: '123e4567-e89b-12d3-a456-426614174001',
  name: 'Size',
  namespace: 'acme',
  published_at: null,
  readonly: false,
  searchable: false,
  status: 'draft',
  tags: [],
  target_blueprint_code: null,
  target_cardinality: null,
  value_schema: { maximum: 10, type: 'number' },
  value_type: 'number',
  version: 1,
  ...overrides,
});

const definitionBox = () =>
  screen.getByRole('textbox', { hidden: true, name: 'Definition' });

const renderPage = async () => {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ReusableAttributeEditorPage />
    </QueryClientProvider>,
  );
  fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
};

beforeEach(() => {
  vi.mocked(listReusableAttributes).mockResolvedValue([]);
  vi.mocked(validateReusableAttribute).mockResolvedValue(checked());
});
afterEach(() => {
  vi.mocked(createReusableAttribute).mockReset();
  vi.mocked(validateReusableAttribute).mockReset();
  navigate.mockReset();
  sessionStorage.clear();
});

describe('ReusableAttributeEditorPage', () => {
  it('starts from a template and asks before replacing edits', async () => {
    await renderPage();
    expect(definitionBox()).toHaveProperty(
      'value',
      reusableAttributeTemplates[0].definition,
    );
    fireEvent.change(definitionBox(), { target: { value: 'code = "x"' } });
    expect(screen.getByText('Unsaved changes')).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Examples' }));
    fireEvent.click(screen.getByRole('button', { name: /Choice/ }));
    const confirm = screen.getByRole('dialog', { name: 'Unsaved changes' });
    fireEvent.click(
      within(confirm).getByRole('button', { name: 'Replace changes' }),
    );
    await waitFor(() =>
      expect(definitionBox()).toHaveProperty(
        'value',
        reusableAttributeTemplates[2].definition,
      ),
    );
  });

  it('asks before discarding edits on cancel', async () => {
    await renderPage();
    fireEvent.change(definitionBox(), { target: { value: 'code = "x"' } });
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    fireEvent.click(screen.getByRole('button', { name: 'Discard changes' }));
    expect(navigate).toHaveBeenCalledWith({
      to: '/manage/reusable-attributes',
    });
  });

  it('previews the checked field and checks sandbox values', async () => {
    await renderPage();
    expect(
      await screen.findByText('Ready to save', {}, { timeout: 2000 }),
    ).toBeTruthy();
    expect(validateReusableAttribute).toHaveBeenCalledWith(
      { definition: reusableAttributeTemplates[0].definition },
      undefined,
      expect.any(AbortSignal),
    );
    expect(screen.getByText('number · acme:size')).toBeTruthy();
    expect(screen.getByText('Explorer filter')).toBeTruthy();
    const field = screen.getByRole('spinbutton', { name: 'Size' });
    fireEvent.change(field, { target: { value: '12' } });
    expect(field.getAttribute('aria-invalid')).toBe('true');
  });

  it('marks a failed check and keeps the last valid preview', async () => {
    await renderPage();
    await screen.findByText('Ready to save', {}, { timeout: 2000 });
    vi.mocked(validateReusableAttribute).mockRejectedValue(
      new ApiRequestError(
        422,
        "attribute 'size' has unsupported value type 'colour'",
        'invalid_reusable_attribute_definition',
      ),
    );
    fireEvent.change(definitionBox(), {
      target: { value: 'code = "size"\nname = "Size"\nvalue_type = "colour"' },
    });
    expect(
      await screen.findByText('Problem on line 3', {}, { timeout: 2000 }),
    ).toBeTruthy();
    expect(screen.getByRole('alert').textContent).toContain(
      "unsupported value type 'colour'",
    );
    expect(
      screen.getByText('Showing the last version that passed the save checks.'),
    ).toBeTruthy();
    expect(screen.getByRole('spinbutton', { name: 'Size' })).toBeTruthy();
  });

  it('freezes the definition and navigation during save', async () => {
    vi.mocked(createReusableAttribute).mockImplementation(
      () => new Promise(() => {}),
    );
    await renderPage();
    fireEvent.change(definitionBox(), {
      target: { value: 'code = "saved"' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Create attribute' }));
    await waitFor(() =>
      expect(createReusableAttribute).toHaveBeenCalledWith({
        definition: 'code = "saved"',
      }),
    );
    expect(definitionBox().hasAttribute('disabled')).toBe(true);
    expect(
      screen.getByRole('button', { name: 'Cancel' }).hasAttribute('disabled'),
    ).toBe(true);
    expect(
      screen.getByRole('button', { name: 'Insert' }).hasAttribute('disabled'),
    ).toBe(true);
  });
});
