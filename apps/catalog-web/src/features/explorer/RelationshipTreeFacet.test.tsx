// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { getRelationshipTreeFacetChildren } from '../entities/api';
import { RelationshipTreeFacet } from './RelationshipTreeFacet';

vi.mock('../entities/api', () => ({
  getRelationshipTreeFacetChildren: vi.fn(),
}));

const firstId = '123e4567-e89b-12d3-a456-426614174000';
const secondId = '123e4567-e89b-12d3-a456-426614174001';
const defaultContext = {
  code: 'default',
  id: '123e4567-e89b-12d3-a456-426614174002',
  data: {},
  parent_id: null,
};

const renderFacet = (
  contexts: Parameters<typeof RelationshipTreeFacet>[0]['contexts'],
  overrides: Partial<Parameters<typeof RelationshipTreeFacet>[0]> = {},
) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const onSelectedIdsChange = vi.fn();
  render(
    <QueryClientProvider client={queryClient}>
      <RelationshipTreeFacet
        blueprint="product"
        contextCode="default"
        contexts={contexts}
        hierarchyFields={[]}
        onHierarchyFieldChange={vi.fn()}
        onSelectedIdsChange={onSelectedIdsChange}
        selectedIds={[]}
        sourceField="category"
        {...overrides}
      />
    </QueryClientProvider>,
  );
  return onSelectedIdsChange;
};

describe('RelationshipTreeFacet', () => {
  it('expands a branch when its item row is clicked', async () => {
    vi.mocked(getRelationshipTreeFacetChildren).mockImplementation((request) =>
      Promise.resolve({
        items: request.parent_id
          ? [
              {
                id: secondId,
                display: 'Child category',
                count: 1,
                has_children: false,
              },
            ]
          : [
              {
                id: firstId,
                display: 'Parent category',
                count: 1,
                has_children: true,
              },
            ],
        selected_items: [],
        next_cursor: null,
      }),
    );
    renderFacet([defaultContext]);
    const user = userEvent.setup();

    await user.click(await screen.findByText('Parent category (1)'));

    expect(await screen.findByText('Child category (1)')).toBeTruthy();
  });

  it.each([
    { contexts: [], state: 'contexts are loading' },
    {
      contexts: [
        {
          code: 'other',
          id: firstId,
          data: {},
          parent_id: null,
        },
      ],
      state: 'the selected context is unknown',
    },
  ])('does not request facet children while $state', ({ contexts }) => {
    renderFacet(contexts);

    expect(getRelationshipTreeFacetChildren).not.toHaveBeenCalled();
  });

  it('pins selected items first and enforces single selection', async () => {
    vi.mocked(getRelationshipTreeFacetChildren).mockResolvedValue({
      items: [
        {
          id: firstId,
          display: 'Selected category',
          count: 1,
          has_children: false,
        },
        {
          id: secondId,
          display: 'Other category',
          count: 1,
          has_children: false,
        },
      ],
      selected_items: [{ id: firstId, display: 'Selected category' }],
      next_cursor: null,
    });
    const onSelectedIdsChange = renderFacet([defaultContext], {
      selectedIds: [firstId],
      singleSelect: true,
    });
    const user = userEvent.setup();

    expect(await screen.findByText('Selected category')).toBeTruthy();
    expect(screen.getAllByText('Selected category')).toHaveLength(1);
    expect(
      screen
        .getByText('Selected')
        .compareDocumentPosition(screen.getByText('Other category (1)')) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    expect(screen.queryByRole('radio')).toBeNull();
    await user.click(
      screen.getByRole('button', { name: 'Select Other category' }),
    );
    expect(onSelectedIdsChange).toHaveBeenLastCalledWith([secondId]);
  });
});
