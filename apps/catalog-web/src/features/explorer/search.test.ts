import { describe, expect, it } from 'vitest';
import { parseExplorerSearch } from './search';

describe('parseExplorerSearch', () => {
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
