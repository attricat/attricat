// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import {
  getBlueprintByCode,
  getEntityPreview,
  searchEntities,
  type Attribute,
  type EntitySearchResponse,
} from '../api';
import { RelationshipField } from './RelationshipField';

vi.mock('../api', () => ({
  getBlueprintByCode: vi.fn(),
  getEntityPreview: vi.fn(),
  searchEntities: vi.fn(),
}));

const firstId = '123e4567-e89b-12d3-a456-426614174000';
const laterId = '123e4567-e89b-12d3-a456-426614174001';
const attribute = {
  code: 'related_products',
  value_type: 'relationship' as const,
  target_blueprint_code: 'product',
};

const page = (
  items: { id: string; label: string }[],
  nextCursor: string | null,
): EntitySearchResponse => ({
  blueprint: {
    blueprint: { code: 'product', name: 'Product', version: 1, views: {} },
    attributes: [],
  },
  items: items.map(({ id, label }) => ({
    id,
    blueprint_version: 1,
    schema_outdated: false,
    display: { default: label },
    match_explanations: [],
    preview: {},
  })),
  next_cursor: nextCursor,
  total_count: null,
  total_count_capped: false,
});

const renderField = (
  onChange = vi.fn(),
  field: Attribute = attribute,
  value = '',
) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={queryClient}>
      <RelationshipField attribute={field} onChange={onChange} value={value} />
    </QueryClientProvider>,
  );
  return onChange;
};

const openSelector = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(
    await screen.findByRole('button', { name: 'related_products' }),
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
        name: 'Product',
        version: 1,
        views: {
          dropdown_option: { type: 'dropdown_option', fields: ['name'] },
        },
      },
      attributes: [],
    });
    vi.mocked(getEntityPreview).mockImplementation((id) =>
      Promise.resolve({
        entity: {
          id,
          blueprint_id: firstId,
          blueprint_version: 1,
        },
        context: { default: { name: `Product ${id.slice(-2)}` } },
      }),
    );

    const onChange = renderField(vi.fn(), attribute, selectedIds.join(', '));
    const user = userEvent.setup();

    expect(await screen.findByText('Product 00')).toBeTruthy();
    expect(screen.getByText(selectedIds[10])).toBeTruthy();
    expect(getEntityPreview).toHaveBeenCalledTimes(10);

    const firstPill = screen.getByRole('button', { name: 'Product 00' });
    await user.click(firstPill.querySelector('.MuiChip-deleteIcon')!);
    expect(onChange).toHaveBeenLastCalledWith(selectedIds.slice(1).join(', '));
  });

  it('keeps multi-selection draft changes in a closeable modal until applied', async () => {
    vi.mocked(searchEntities).mockImplementation((_, __, query, cursor) =>
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
    expect(searchEntities).toHaveBeenCalledWith(
      'product',
      undefined,
      'later',
      'second-page',
      undefined,
      expect.any(AbortSignal),
    );
  });

  it('discards draft changes when the modal is closed', async () => {
    vi.mocked(searchEntities).mockResolvedValue(
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
    vi.mocked(searchEntities).mockResolvedValue(
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
      relationship_cardinality: 'one_to_one',
    });
    const user = userEvent.setup();

    await user.click(
      await screen.findByRole('button', { name: 'related_products' }),
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
