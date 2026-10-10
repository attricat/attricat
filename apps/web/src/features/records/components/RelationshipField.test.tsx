// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import {
  getBlueprintByCode,
  getRecordPreview,
  searchRecords,
  type Attribute,
  type RecordSearchResponse,
} from '../api';
import { RelationshipField } from './RelationshipField';

vi.mock('../api', () => ({
  getBlueprintByCode: vi.fn(),
  getRecordPreview: vi.fn(),
  searchRecords: vi.fn(),
}));

const blueprintId = '123e4567-e89b-12d3-a456-426614174000';
const firstId = '123e4567-e89b-12d3-a456-426614174001';
const laterId = '123e4567-e89b-12d3-a456-426614174002';
const attribute = {
  code: 'related_products',
  value_type: 'relationship' as const,
  target_blueprint_code: 'product',
};

const page = (
  items: { id: string; label: string; isSample?: boolean }[],
  nextCursor: string | null,
): RecordSearchResponse => ({
  blueprint: {
    blueprint: {
      code: 'product',
      id: blueprintId,
      name: 'Product',
      status: 'published',
      version: 1,
      views: {},
    },
    attributes: [],
    table_path_attributes: [],
  },
  items: items.map(({ id, label, isSample = false }) => ({
    id,
    blueprint_version: 1,
    schema_outdated: false,
    is_sample: isSample,
    display: { default: label },
    match_explanations: [],
    preview: {},
    table_values: {},
  })),
  next_cursor: nextCursor,
  total_count: null,
  total_count_capped: false,
  result_version_scope: { kind: 'single', version: 1 },
  hidden_outdated_count: null,
  hidden_outdated_count_capped: false,
});

const renderField = (
  onChange = vi.fn(),
  field: Attribute = attribute,
  value = '',
  disabled = false,
) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={queryClient}>
      <RelationshipField
        attribute={field}
        disabled={disabled}
        onChange={onChange}
        value={value}
      />
    </QueryClientProvider>,
  );
  return onChange;
};

const openSelector = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(
    await screen.findByRole('button', { name: 'Choose related products' }),
  );
  return screen.findByRole('dialog', { name: /^Select product/ });
};

describe('RelationshipField', () => {
  it('resolves up to ten selected IDs into name pills', async () => {
    const selectedIds = Array.from(
      { length: 11 },
      (_, index) =>
        `123e4567-e89b-12d3-a456-${String(index).padStart(12, '0')}`,
    );
    vi.mocked(getBlueprintByCode).mockResolvedValue({
      blueprint: {
        code: 'product',
        id: blueprintId,
        name: 'Product',
        status: 'published',
        version: 1,
        views: {
          dropdown_option: { type: 'dropdown_option', fields: ['name'] },
        },
      },
      attributes: [],
      table_path_attributes: [],
    });
    vi.mocked(getRecordPreview).mockImplementation((id) =>
      Promise.resolve({
        record: {
          id,
          blueprint_id: blueprintId,
          blueprint_version: 1,
          is_sample: false,
        },
        context: { default: { name: `Product ${id.slice(-2)}` } },
      }),
    );

    const onChange = renderField(vi.fn(), attribute, selectedIds.join(', '));
    const user = userEvent.setup();

    expect(await screen.findByText('Product 00')).toBeTruthy();
    expect(screen.getByText(selectedIds[10])).toBeTruthy();
    expect(getRecordPreview).toHaveBeenCalledTimes(10);

    const firstPill = screen.getByRole('button', { name: 'Product 00' });
    await user.click(firstPill.querySelector('.MuiChip-deleteIcon')!);
    expect(onChange).toHaveBeenLastCalledWith(selectedIds.slice(1).join(', '));
  });

  it('does not allow selected relationships to be removed when disabled', async () => {
    vi.mocked(getBlueprintByCode).mockResolvedValue({
      blueprint: {
        code: 'product',
        id: blueprintId,
        name: 'Product',
        status: 'published',
        version: 1,
        views: {},
      },
      attributes: [],
      table_path_attributes: [],
    });
    vi.mocked(getRecordPreview).mockResolvedValue({
      record: {
        id: firstId,
        blueprint_id: blueprintId,
        blueprint_version: 1,
        is_sample: false,
      },
      context: {},
    });
    const onChange = renderField(vi.fn(), attribute, firstId, true);

    const pill = await screen.findByText(firstId);
    expect(pill.parentElement?.querySelector('.MuiChip-deleteIcon')).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('keeps multi-selection draft changes in a closeable modal until applied', async () => {
    vi.mocked(searchRecords).mockImplementation(({ query, cursor }) =>
      Promise.resolve(
        cursor === 'second-page'
          ? page([{ id: laterId, label: 'Later product' }], null)
          : page(
              [{ id: firstId, label: `${query || 'First'} product` }],
              'second-page',
            ),
      ),
    );
    const onChange = renderField();
    const user = userEvent.setup();

    await openSelector(user);
    const search = screen.getByRole('textbox', { name: 'Search options' });
    await user.type(search, 'later');
    await screen.findByRole('button', { name: 'Select later product' });
    const recordLink = screen.getByRole('link', { name: 'later product' });
    expect(recordLink.getAttribute('target')).toBe('_blank');
    const openPreview = vi.spyOn(window, 'open').mockImplementation(() => null);
    const previewButton = screen.getByRole('button', {
      name: 'Preview later product in a new tab',
    });
    await user.click(previewButton);
    expect(openPreview).toHaveBeenCalledWith(
      expect.stringMatching(
        new RegExp(`^/records/${firstId}\\?relationshipPicker=`),
      ),
      '_blank',
    );
    expect(previewButton.className).toContain('MuiButton-colorSecondary');
    openPreview.mockRestore();

    await user.click(screen.getByRole('button', { name: 'Load more' }));
    await user.click(
      await screen.findByRole('button', { name: 'Select Later product' }),
    );

    expect(onChange).not.toHaveBeenCalled();
    expect(
      screen
        .getByText('Selected')
        .compareDocumentPosition(screen.getByText('Options')) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Apply' }));

    expect(onChange).toHaveBeenLastCalledWith(laterId);
    expect(searchRecords).toHaveBeenCalledWith({
      blueprint: 'product',
      cursor: 'second-page',
      query: 'later',
      signal: expect.any(AbortSignal),
    });
  });

  it('searches one allowed target blueprint at a time', async () => {
    vi.mocked(searchRecords).mockResolvedValue(
      page([{ id: firstId, label: 'First option' }], null),
    );
    renderField(vi.fn(), {
      ...attribute,
      target_blueprint_code: null,
      target_blueprint_codes: ['product', 'material'],
    });
    const user = userEvent.setup();

    await user.click(
      await screen.findByRole('button', { name: 'Choose related products' }),
    );
    const dialog = await screen.findByRole('dialog', {
      name: /^Select product, material/,
    });
    await waitFor(() =>
      expect(searchRecords).toHaveBeenLastCalledWith(
        expect.objectContaining({ blueprint: 'product' }),
      ),
    );
    await user.click(
      within(dialog).getByRole('combobox', { name: 'Target blueprint' }),
    );
    await user.click(await screen.findByRole('option', { name: 'material' }));
    await waitFor(() =>
      expect(searchRecords).toHaveBeenLastCalledWith(
        expect.objectContaining({ blueprint: 'material' }),
      ),
    );
  });

  it('labels sample relationship options', async () => {
    vi.mocked(searchRecords).mockResolvedValue(
      page([{ id: firstId, label: 'First product', isSample: true }], null),
    );
    renderField();
    const user = userEvent.setup();

    await openSelector(user);
    expect(await screen.findByText('Sample')).toBeTruthy();
  });

  it('discards draft changes when the modal is closed', async () => {
    vi.mocked(searchRecords).mockResolvedValue(
      page([{ id: firstId, label: 'First product' }], null),
    );
    const onChange = renderField();
    const user = userEvent.setup();

    await openSelector(user);
    await user.click(
      await screen.findByRole('button', { name: 'Select First product' }),
    );
    await user.click(
      screen.getByRole('button', { name: 'Close relationship selector' }),
    );

    expect(onChange).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('allows only one draft selection for one-to-one relationships', async () => {
    vi.mocked(searchRecords).mockResolvedValue(
      page(
        [
          { id: firstId, label: 'First product' },
          { id: laterId, label: 'Later product' },
        ],
        null,
      ),
    );
    const onChange = renderField(vi.fn(), {
      ...attribute,
      cardinality: 'one',
      target_cardinality: 'many',
    });
    const user = userEvent.setup();

    await user.click(
      await screen.findByRole('button', { name: 'Choose related products' }),
    );
    await screen.findByRole('dialog', { name: /^Select one product/ });
    expect(screen.queryByRole('radio')).toBeNull();
    await user.click(
      await screen.findByRole('button', { name: 'Select First product' }),
    );
    await user.click(
      screen.getByRole('button', { name: 'Select Later product' }),
    );

    const selectedSection = screen.getByText('Selected').parentElement!;
    expect(
      within(selectedSection).getByRole('button', { name: 'Later product' }),
    ).toBeTruthy();
    expect(
      within(selectedSection).queryByRole('button', { name: 'First product' }),
    ).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Apply' }));
    expect(onChange).toHaveBeenLastCalledWith(laterId);
  });
});
