import { describe, expect, it } from 'vitest';
import {
  relationshipFiltersFromSearch,
  relationshipTreeFacetsRequest,
  relationshipTreeSelectionsFromSearch,
  updateRelationshipFacets,
} from './explorerSearchState';
import type { ExplorerSearch } from './search';

const brandId = '123e4567-e89b-12d3-a456-426614174001';
const shoesId = '123e4567-e89b-12d3-a456-426614174002';
const contextId = '123e4567-e89b-12d3-a456-426614174003';

const search: ExplorerSearch = {
  blueprint: 'product',
  relationshipFacets: [
    { field: 'brand', selectedIds: [brandId] },
    { field: 'categories', hierarchyField: 'parent', selectedIds: [shoesId] },
  ],
};

describe('relationship facet requests', () => {
  it('sends hierarchy selections as tree facets so descendants match', () => {
    expect(relationshipFiltersFromSearch(search)).toEqual([
      { field: 'brand', selected_target_ids: [brandId] },
    ]);
    expect(
      relationshipTreeFacetsRequest(
        relationshipTreeSelectionsFromSearch(search),
        contextId,
      ),
    ).toEqual([
      {
        context_id: contextId,
        hierarchy_field: 'parent',
        selected_target_ids: [shoesId],
        source_relationship_field: 'categories',
      },
    ]);
  });

  it('sends an own-tree selection without a source relationship', () => {
    expect(
      relationshipTreeFacetsRequest(
        relationshipTreeSelectionsFromSearch({
          blueprint: 'category',
          relationshipFacets: [
            {
              field: 'parent',
              hierarchyField: 'parent',
              ownTree: true,
              selectedIds: [shoesId],
            },
          ],
        }),
        contextId,
      ),
    ).toEqual([
      {
        context_id: contextId,
        hierarchy_field: 'parent',
        selected_target_ids: [shoesId],
      },
    ]);
  });

  it('drops a facet, hierarchy included, when its selection is cleared', () => {
    expect(
      updateRelationshipFacets(search.relationshipFacets, 'categories', {
        hierarchyField: 'parent',
        selectedIds: [],
      }),
    ).toEqual([{ field: 'brand', selectedIds: [brandId] }]);
  });
});
