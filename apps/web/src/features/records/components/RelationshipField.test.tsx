// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import {
  getRecordLabels,
  listRecordBlueprints,
  searchRecords,
  type Attribute,
  type RecordSearchResponse,
} from '../api';
import { RelationshipField } from './RelationshipField';
import { relationshipPickerMessageType } from './useRecentlyPreviewedRecords';

vi.mock('../api', () => ({
  getRecordLabels: vi.fn(),
  listRecordBlueprints: vi.fn(),
  searchRecords: vi.fn(),
}));

const blueprintId = '123e4567-e89b-12d3-a456-426614174000';
const firstId = '123e4567-e89b-12d3-a456-426614174001';
const laterId = '123e4567-e89b-12d3-a456-426614174002';
const unlabelledId = '123e4567-e89b-12d3-a456-426614174003';
const attribute = {
  code: 'related_products',
  value_type: 'relationship' as const,
  target_blueprint_code: 'product',
};

const recordBlueprint = (code: string, name: string) => ({
  code,
  id: blueprintId,
  name,
  status: 'published' as const,
  version: 1,
  views: {},
});

/** Labels each requested record from `labels`, leaving the rest unlabelled. */
const mockLabels = (labels: Record<string, string>) =>
  vi.mocked(getRecordLabels).mockImplementation((ids) =>
    Promise.resolve({
      items: ids.flatMap((id) =>
        labels[id] === undefined
          ? []
          : [
              {
                id,
                blueprint_code: 'product',
                display: { default: labels[id] },
              },
            ],
      ),
    }),
  );

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
    system_tags: [],
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
  return screen.findByRole('dialog', { name: /^Select product/i });
};

describe('RelationshipField', () => {
  it('labels selected records of different blueprints in one lookup', async () => {
    mockLabels({ [firstId]: 'Oak chair', [laterId]: 'Walnut veneer' });

    const onChange = renderField(
      vi.fn(),
      attribute,
      [firstId, laterId, unlabelledId].join(', '),
    );
    const user = userEvent.setup();

    expect(await screen.findByText('Oak chair')).toBeTruthy();
    expect(screen.getByText('Walnut veneer')).toBeTruthy();
    expect(screen.getByText(unlabelledId)).toBeTruthy();
    expect(getRecordLabels).toHaveBeenCalledTimes(1);

    const firstPill = screen.getByRole('button', { name: 'Oak chair' });
    await user.click(firstPill.querySelector('.MuiChip-deleteIcon')!);
    expect(onChange).toHaveBeenLastCalledWith(
      [laterId, unlabelledId].join(', '),
    );
  });

  it('does not allow selected relationships to be removed when disabled', async () => {
    mockLabels({});
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
    await screen.findByRole('checkbox', { name: 'later product' });
    const preview = screen.getByRole('link', {
      name: 'Preview later product in a new tab',
    });
    expect(preview.getAttribute('target')).toBe('_blank');
    expect(preview.getAttribute('href')).toMatch(
      new RegExp(`^/records/${firstId}\\?relationshipPicker=`),
    );
    fireEvent.click(preview);
    expect(preview.className).toContain('MuiIconButton-colorSecondary');

    await user.click(screen.getByRole('button', { name: 'Load more' }));
    const later = await screen.findByRole('checkbox', {
      name: 'Later product',
    });
    await user.click(later);
    expect((later as HTMLInputElement).checked).toBe(true);

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
    vi.mocked(listRecordBlueprints).mockResolvedValue([
      recordBlueprint('product', 'Product'),
      recordBlueprint('material', 'Material'),
      recordBlueprint('supplier', 'Supplier'),
    ]);
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
      name: /^Select Product, Material/,
    });
    await waitFor(() =>
      expect(searchRecords).toHaveBeenLastCalledWith(
        expect.objectContaining({ blueprint: 'product' }),
      ),
    );
    await user.click(
      within(dialog).getByRole('combobox', { name: 'Target blueprint' }),
    );
    await user.click(await screen.findByRole('option', { name: 'Material' }));
    await waitFor(() =>
      expect(searchRecords).toHaveBeenLastCalledWith(
        expect.objectContaining({ blueprint: 'material' }),
      ),
    );
    expect(screen.queryByRole('option', { name: 'Supplier' })).toBeNull();
  });

  it('searches every record blueprint when the relationship names no target', async () => {
    vi.mocked(listRecordBlueprints).mockResolvedValue([
      recordBlueprint('product', 'Product'),
      recordBlueprint('material', 'Material'),
    ]);
    vi.mocked(searchRecords).mockResolvedValue(
      page([{ id: firstId, label: 'First option' }], null),
    );
    renderField(vi.fn(), {
      ...attribute,
      target_blueprint_code: null,
      target_blueprint_codes: [],
    });
    const user = userEvent.setup();

    expect(screen.queryByRole('textbox')).toBeNull();
    const selector = await screen.findByRole('button', {
      name: 'Choose related products',
    });
    await waitFor(() =>
      expect((selector as HTMLButtonElement).disabled).toBe(false),
    );
    await user.click(selector);
    const dialog = await screen.findByRole('dialog', {
      name: /^Select records/,
    });
    await waitFor(() =>
      expect(searchRecords).toHaveBeenLastCalledWith(
        expect.objectContaining({ blueprint: 'product' }),
      ),
    );
    await user.click(
      within(dialog).getByRole('combobox', { name: 'Target blueprint' }),
    );
    expect(await screen.findByRole('option', { name: 'Product' })).toBeTruthy();
    await user.click(screen.getByRole('option', { name: 'Material' }));
    await waitFor(() =>
      expect(searchRecords).toHaveBeenLastCalledWith(
        expect.objectContaining({ blueprint: 'material' }),
      ),
    );
  });

  it('names no blueprint when one record of any blueprint is picked', async () => {
    vi.mocked(listRecordBlueprints).mockResolvedValue([
      recordBlueprint('product', 'Product'),
    ]);
    vi.mocked(searchRecords).mockResolvedValue(page([], null));
    renderField(vi.fn(), {
      ...attribute,
      cardinality: 'one',
      target_blueprint_code: null,
      target_blueprint_codes: [],
    });
    const user = userEvent.setup();

    const selector = await screen.findByRole('button', {
      name: 'Choose related products',
    });
    await waitFor(() =>
      expect((selector as HTMLButtonElement).disabled).toBe(false),
    );
    await user.click(selector);
    expect(
      await screen.findByRole('dialog', { name: /^Select one record/ }),
    ).toBeTruthy();
  });

  it('disables the selector until some record blueprint can be targeted', async () => {
    vi.mocked(listRecordBlueprints).mockResolvedValue([]);
    renderField(vi.fn(), {
      ...attribute,
      target_blueprint_code: null,
      target_blueprint_codes: [],
    });

    const selector = await screen.findByRole('button', {
      name: 'Choose related products',
    });
    expect((selector as HTMLButtonElement).disabled).toBe(true);
    await waitFor(() => expect(listRecordBlueprints).toHaveBeenCalled());
    expect((selector as HTMLButtonElement).disabled).toBe(true);
  });

  it('keeps previews from an earlier opening of the picker', async () => {
    // jsdom does not scroll; selecting scrolls the picker to the top.
    Element.prototype.scrollTo = vi.fn();
    vi.mocked(searchRecords).mockResolvedValue(
      page([{ id: firstId, label: 'First product' }], null),
    );
    renderField();
    const user = userEvent.setup();

    await openSelector(user);
    const preview = await screen.findByRole('link', {
      name: 'Preview First product in a new tab',
    });
    const href = preview.getAttribute('href') ?? '';
    fireEvent.click(preview);
    await user.click(
      screen.getByRole('button', { name: 'Close relationship selector' }),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());

    const dialog = await openSelector(user);
    const reopened = await within(dialog).findByRole('link', {
      name: 'Preview First product in a new tab',
    });
    expect(reopened.getAttribute('href')).toBe(href);
    expect(reopened.className).toContain('MuiIconButton-colorSecondary');
    window.dispatchEvent(
      new MessageEvent('message', {
        data: {
          recordId: firstId,
          token: new URL(href, location.href).searchParams.get(
            'relationshipPicker',
          ),
          type: relationshipPickerMessageType,
        },
        origin: window.location.origin,
      }),
    );
    await waitFor(() => expect(dialog.textContent).toContain('1 selected'));
  });

  it('names the blueprint whose records could not be loaded', async () => {
    vi.mocked(listRecordBlueprints).mockResolvedValue([
      recordBlueprint('product', 'Catalog product'),
    ]);
    vi.mocked(searchRecords).mockRejectedValue(new Error('offline'));
    renderField();
    const user = userEvent.setup();

    await user.click(
      await screen.findByRole('button', { name: 'Choose related products' }),
    );
    await screen.findByRole('dialog', { name: /^Select Catalog product/ });

    expect(
      await screen.findByText('Could not load Catalog product records.'),
    ).toBeTruthy();
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
      await screen.findByRole('checkbox', { name: 'First product' }),
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
    await screen.findByRole('dialog', { name: /^Select one product/i });
    expect(screen.queryByRole('checkbox')).toBeNull();
    await user.click(
      await screen.findByRole('radio', { name: 'First product' }),
    );
    await user.click(screen.getByRole('radio', { name: 'Later product' }));
    expect(
      (screen.getByRole('radio', { name: 'First product' }) as HTMLInputElement)
        .checked,
    ).toBe(false);

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
