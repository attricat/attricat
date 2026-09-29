import { describe, expect, it } from 'vitest';
import type { Attribute } from '../entities/api';
import {
  cellFilterDraft,
  cellFilterValueType,
  cellValueFilter,
} from './cellFilters';

const attribute = (value_type: Attribute['value_type']): Attribute => ({
  code: 'title',
  value_type,
});

describe('cell filters', () => {
  it('resolves direct and relationship path value types', () => {
    expect(cellFilterValueType('title', attribute('string'), [])).toBe(
      'string',
    );
    expect(cellFilterValueType('title', attribute('json'), [])).toBeUndefined();
    expect(
      cellFilterValueType('brand.name', attribute('relationship'), [
        { code: 'brand.name', value_type: 'string' },
      ]),
    ).toBe('string');
    expect(
      cellFilterValueType('brand.name', attribute('relationship'), []),
    ).toBeUndefined();
  });

  it('builds equality filters only for single scalar values of the column type', () => {
    expect(cellValueFilter('title', 'string', ['Laptop'])).toEqual({
      field: 'title',
      operator: 'eq',
      value: 'Laptop',
    });
    expect(cellValueFilter('price', 'number', [12.5])?.value).toBe(12.5);
    expect(cellValueFilter('active', 'boolean', [false])?.value).toBe(false);
    expect(cellValueFilter('title', 'string', [])).toBeUndefined();
    expect(cellValueFilter('title', 'string', [''])).toBeUndefined();
    expect(cellValueFilter('title', 'string', ['a', 'b'])).toBeUndefined();
    expect(cellValueFilter('title', 'string', [null])).toBeUndefined();
    expect(cellValueFilter('price', 'number', ['12'])).toBeUndefined();
  });

  it('converts datetime values into drafts in the preferred zone', () => {
    const filter = cellValueFilter('seen_at', 'datetime', [
      '2026-01-02T03:04:00.000Z',
    ]);
    expect(
      filter && cellFilterDraft(filter, 'datetime', 'Europe/Warsaw'),
    ).toEqual({
      field: 'seen_at',
      operator: 'eq',
      value: '2026-01-02T04:04',
    });
    expect(filter && cellFilterDraft(filter, 'string', 'UTC').value).toBe(
      '2026-01-02T03:04:00.000Z',
    );
  });
});
