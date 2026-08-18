import { describe, expect, it } from 'vitest';
import { parseExplorerSearch } from './search';

describe('parseExplorerSearch', () => {
  it('retains valid URL state and drops invalid version values', () => {
    expect(
      parseExplorerSearch({
        blueprint: 'product',
        version: '3',
        query: 'shirt',
      }),
    ).toEqual({ blueprint: 'product', version: 3, query: 'shirt' });
    expect(parseExplorerSearch({ blueprint: '', version: 'zero' })).toEqual({
      blueprint: undefined,
      version: undefined,
      query: undefined,
    });
  });

  it('trims valid string values', () => {
    expect(
      parseExplorerSearch({ blueprint: ' product ', query: ' shirt ' }),
    ).toEqual({
      blueprint: 'product',
      query: 'shirt',
    });
  });
});
