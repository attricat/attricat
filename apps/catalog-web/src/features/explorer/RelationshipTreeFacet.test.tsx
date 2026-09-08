// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { getRelationshipTreeFacetChildren } from '../entities/api';
import { RelationshipTreeFacet } from './RelationshipTreeFacet';

vi.mock('../entities/api', () => ({
  getRelationshipTreeFacetChildren: vi.fn(),
}));

const renderFacet = (
  contexts: Parameters<typeof RelationshipTreeFacet>[0]['contexts'],
) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <RelationshipTreeFacet
        blueprint="product"
        contextCode="default"
        contexts={contexts}
        hierarchyFields={[]}
        onHierarchyFieldChange={vi.fn()}
        onSelectedIdsChange={vi.fn()}
        selectedIds={[]}
        sourceField="category"
      />
    </QueryClientProvider>,
  );
};

describe('RelationshipTreeFacet', () => {
  it.each([
    { contexts: [], state: 'contexts are loading' },
    {
      contexts: [
        {
          code: 'other',
          id: '123e4567-e89b-12d3-a456-426614174000',
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
});
