import type { Attribute } from '../records/api';
import { relationshipFacetSources } from './explorerSearchState';
import { describe, expect, it } from 'vitest';
import {
  dataQualityRequest,
  explorerResultsQueryKey,
  exportStatusRequest,
  relationshipFiltersFromSearch,
  relationshipTreeFacetsRequest,
  relationshipTreeSelectionsFromSearch,
  submittedExplorerSearch,
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

describe('submittedExplorerSearch', () => {
  const facets = [
    { field: 'brand.owner', selectedIds: [brandId], targetBlueprint: 'owner' },
  ];

  it('treats the current version named explicitly as the same scope', () => {
    const { nextSearch, scopeChanged } = submittedExplorerSearch(
      { blueprint: 'product', version: 3, relationshipFacets: facets },
      { blueprint: 'product', query: 'shoe' },
      3,
    );
    expect(scopeChanged).toBe(false);
    expect(nextSearch.relationshipFacets).toEqual(facets);
    expect(nextSearch.query).toBe('shoe');
  });

  it('reports a change to another revision or to every version', () => {
    const current = { blueprint: 'product', version: 3 };
    expect(
      submittedExplorerSearch(current, { blueprint: 'product', version: 2 }, 3)
        .scopeChanged,
    ).toBe(true);
    expect(
      submittedExplorerSearch(
        current,
        { blueprint: 'product', allVersions: true },
        3,
      ).scopeChanged,
    ).toBe(true);
  });
});

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

  it('drops a malformed connection from the URL', () => {
    const parsed = parseExplorerSearch({
      blueprint: 'product',
      exportStatus: {
        extension: 'acme.shop',
        connection: 'shop eu',
        status: 'rejected',
      },
    });
    expect(parsed.exportStatus).toBeUndefined();
  });
});

describe('data quality filter', () => {
  const filtered: ExplorerSearch = {
    blueprint: 'product',
    dataQuality: 'open',
  };
  const key = (canReadRules: boolean) =>
    explorerResultsQueryKey(
      filtered,
      [],
      1,
      undefined,
      'default',
      canReadRules,
    );

  it('is sent and cached apart for viewers who may read rules', () => {
    expect(dataQualityRequest(filtered, true)).toBe('open');
    expect(key(true)[1]).toMatchObject({ dataQuality: 'open' });
  });

  it('is dropped for viewers who may not read rules', () => {
    // A link or saved view may carry the filter; the server would refuse it
    // and these viewers have no chip to remove it.
    expect(dataQualityRequest(filtered, false)).toBeUndefined();
    expect(key(false)).toEqual(
      explorerResultsQueryKey(
        { blueprint: 'product' },
        [],
        1,
        undefined,
        'default',
        false,
      ),
    );
  });
});

describe('relationshipFacetSources', () => {
  it('finds the attribute of a facet on a field hidden from the filters', () => {
    const hidden = {
      code: 'made_by',
      target_blueprint_code: 'supplier',
      value_type: 'relationship',
    } as Attribute;
    expect(
      relationshipFacetSources(
        [],
        {
          blueprint: 'product',
          relationshipFacets: [{ field: 'made_by', selectedIds: ['a'] }],
        },
        [hidden],
      ),
    ).toEqual([hidden]);
  });
});
