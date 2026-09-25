import { describe, expect, it } from 'vitest';
import { inlineExplorerSearchParams, parseExplorerSearch } from './search';

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
