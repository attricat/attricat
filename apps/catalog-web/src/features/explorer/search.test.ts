import { describe, expect, it } from 'vitest';
import {
  entitySelectionSearch,
  inlineExplorerSearchParams,
  parseExplorerRouteSearch,
  parseExplorerSearch,
} from './search';

describe('parseExplorerSearch', () => {
  it('serializes inline links without saved-view references', () => {
    const params = inlineExplorerSearchParams({
      blueprint: 'product',
      allVersions: false,
      sourceView: 'b67f5d16-d2be-4669-9870-b5a73282a26e',
      sort: { field: 'name', direction: 'asc' },
      attributeFilters: [{ field: 'price', operator: 'gte', value: 100 }],
    });
    expect(params.get('blueprint')).toBe('product');
    expect(params.get('allVersions')).toBe('false');
    expect(params.get('sort')).toBe('{"field":"name","direction":"asc"}');
    expect(params.get('attributeFilters')).toBe(
      '[{"field":"price","operator":"gte","value":100}]',
    );
    expect(params.has('sourceView')).toBe(false);
  });

  it('accepts saved-view references and drops malformed identifiers', () => {
    const id = 'b67f5d16-d2be-4669-9870-b5a73282a26e';
    expect(
      parseExplorerSearch({ savedView: id, viewState: 'invalid' }),
    ).toEqual({
      savedView: id,
      viewState: undefined,
    });
    expect(parseExplorerSearch({ viewState: id })).toEqual({ viewState: id });
  });

  it('retains valid URL state and drops invalid version values', () => {
    expect(
      parseExplorerSearch({
        blueprint: 'product',
        version: '3',
        allVersions: true,
        query: 'shirt',
      }),
    ).toEqual({
      blueprint: 'product',
      version: 3,
      allVersions: true,
      query: 'shirt',
    });
    expect(parseExplorerSearch({ blueprint: '', version: 'zero' })).toEqual({
      blueprint: undefined,
      version: undefined,
      query: undefined,
    });
  });

  it('preserves valid table sort state and drops malformed values', () => {
    expect(
      parseExplorerSearch({
        blueprint: 'product',
        sort: { field: 'category.name', direction: 'desc' },
      }),
    ).toEqual({
      blueprint: 'product',
      sort: { field: 'category.name', direction: 'desc' },
    });
    expect(
      parseExplorerSearch({
        blueprint: 'product',
        sort: { field: '', direction: 'up' },
      }),
    ).toEqual({ blueprint: 'product', sort: undefined });
  });

  it('preserves valid attribute filters and drops malformed filter state', () => {
    expect(
      parseExplorerSearch({
        blueprint: 'product',
        attributeFilters: [
          { field: 'price', operator: 'gte', value: 100 },
          { field: 'available', operator: 'eq', value: true },
        ],
      }),
    ).toEqual({
      blueprint: 'product',
      attributeFilters: [
        { field: 'price', operator: 'gte', value: 100 },
        { field: 'available', operator: 'eq', value: true },
      ],
    });
    expect(
      parseExplorerSearch({
        blueprint: 'product',
        attributeFilters: [{ field: '', operator: 'nope', value: {} }],
      }),
    ).toEqual({ blueprint: 'product', attributeFilters: undefined });
  });

  it('preserves locked mode for pinned navigation shortcuts', () => {
    expect(parseExplorerSearch({ blueprint: 'product', locked: true })).toEqual(
      {
        blueprint: 'product',
        locked: true,
      },
    );
  });

  it('trims valid string values', () => {
    expect(
      parseExplorerSearch({
        blueprint: ' product ',
        context: ' storefront ',
        query: ' shirt ',
      }),
    ).toEqual({
      blueprint: 'product',
      context: 'storefront',
      query: 'shirt',
    });
  });
});

describe('entitySelectionSearch', () => {
  it('matches only the selected entities within the same scope', () => {
    expect(
      entitySelectionSearch(
        {
          blueprint: 'product',
          allVersions: true,
          context: 'web',
          query: 'shoe',
          sourceView: 'b67f5d16-d2be-4669-9870-b5a73282a26e',
          sort: { field: 'name', direction: 'asc' },
          attributeFilters: [{ field: 'price', operator: 'gte', value: 100 }],
          relationshipFacets: [{ field: 'brand', selectedIds: [] }],
        },
        [
          '123e4567-e89b-12d3-a456-426614174001',
          '123e4567-e89b-12d3-a456-426614174002',
        ],
      ),
    ).toEqual({
      blueprint: 'product',
      allVersions: true,
      context: 'web',
      query:
        '@id:123e4567-e89b-12d3-a456-426614174001,123e4567-e89b-12d3-a456-426614174002',
      sort: { field: 'name', direction: 'asc' },
    });
  });
});

describe('parseExplorerRouteSearch', () => {
  it('keeps the panel entity outside the Explorer search', () => {
    const entity = 'b67f5d16-d2be-4669-9870-b5a73282a26e';
    expect(parseExplorerRouteSearch({ blueprint: 'product', entity })).toEqual({
      blueprint: 'product',
      entity,
    });
    expect(parseExplorerSearch({ blueprint: 'product', entity })).toEqual({
      blueprint: 'product',
    });
  });

  it('drops a malformed panel entity', () => {
    expect(parseExplorerRouteSearch({ entity: 'invalid' })).toEqual({});
  });
});
