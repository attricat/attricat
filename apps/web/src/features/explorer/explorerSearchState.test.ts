import { describe, expect, it } from 'vitest';
import {
  exportStatusRequest,
  relationshipFiltersFromSearch,
  relationshipTreeFacetsRequest,
  relationshipTreeSelectionsFromSearch,
  updateRelationshipFacets,
} from './explorerSearchState';
import { parseExplorerSearch, type ExplorerSearch } from './search';

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
  it('sends Browse rail paths like direct fields', () => {
    const paths: ExplorerSearch = {
      blueprint: 'vehicle',
      browseBy: 'model.body_type',
      relationshipFacets: [
        { field: 'model.brand', selectedIds: [brandId] },
        {
          field: 'model.body_type',
          hierarchyField: 'parent',
          selectedIds: [shoesId],
          targetBlueprint: 'body_type',
        },
      ],
    };
    expect(relationshipFiltersFromSearch(paths)).toEqual([
      { field: 'model.brand', selected_target_ids: [brandId] },
    ]);
    expect(
      relationshipTreeFacetsRequest(
        relationshipTreeSelectionsFromSearch(paths),
        contextId,
      ),
    ).toEqual([
      {
        context_id: contextId,
        hierarchy_field: 'parent',
        selected_target_ids: [shoesId],
        source_relationship_field: 'model.body_type',
      },
    ]);
  });

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

describe('export status filter', () => {
  it('round-trips through the URL and becomes the request field', () => {
    const parsed = parseExplorerSearch({
      blueprint: 'product',
      exportStatus: {
        extension: 'acme.shop',
        connection: 'shop-eu',
        status: 'rejected',
      },
    });
    expect(exportStatusRequest(parsed)).toEqual({
      extension_id: 'acme.shop',
      connection: 'shop-eu',
      status: 'rejected',
    });
    expect(
      exportStatusRequest(
        parseExplorerSearch({
          blueprint: 'product',
          exportStatus: { extension: 'acme.shop', status: 'synced' },
        }),
      ),
    ).toEqual({ extension_id: 'acme.shop', status: 'synced' });
  });

  it('drops an unknown status from the URL', () => {
    const parsed = parseExplorerSearch({
      blueprint: 'product',
      exportStatus: { extension: 'acme.shop', status: 'exploded' },
    });
    expect(parsed.exportStatus).toBeUndefined();
    expect(exportStatusRequest(parsed)).toBeUndefined();
  });
});
