// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { searchEntities, type EntitySearchResponse } from '../api';
import { RelationshipField } from './RelationshipField';

vi.mock('../api', () => ({
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
});

const renderField = (onChange = vi.fn()) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={queryClient}>
      <RelationshipField attribute={attribute} onChange={onChange} value="" />
    </QueryClientProvider>,
  );
  return onChange;
};

describe('RelationshipField', () => {
  it('loads and selects a target from a later server search page', async () => {
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

    const input = await screen.findByRole('combobox', {
      name: 'related_products',
    });
    await user.type(input, 'later');
    await screen.findByRole('option', { name: 'later product' });

    await user.click(screen.getByRole('button', { name: 'Load more' }));
    await user.click(input);
    const laterOption = await screen.findByRole('option', {
      name: 'Later product',
    });
    await user.click(laterOption);

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
});
