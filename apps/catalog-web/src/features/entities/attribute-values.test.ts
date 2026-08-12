import { describe, expect, it } from 'vitest';
import { scalarValueForField, valueForField } from './attribute-values';

describe('attribute values', () => {
  it('formats API scalar values for form fields', () => {
    expect(valueForField(false)).toBe('false');
    expect(valueForField({ time: '09:30:00', time_zone: 'America/New_York' })).toBe(
      '09:30:00 America/New_York',
    );
  });

  it('parses fields into their declared scalar types', () => {
    expect(
      scalarValueForField({ code: 'available', value_type: 'boolean' }, 'false'),
    ).toEqual({ kind: 'scalar', attribute_code: 'available', value: false });
    expect(
      scalarValueForField({ code: 'price', value_type: 'number' }, '49.95'),
    ).toEqual({ kind: 'scalar', attribute_code: 'price', value: 49.95 });
    expect(
      scalarValueForField({ code: 'cutoff', value_type: 'time' }, '09:30 America/New_York'),
    ).toEqual({
      kind: 'scalar',
      attribute_code: 'cutoff',
      value: { time: '09:30', time_zone: 'America/New_York' },
    });
  });

  it('does not silently coerce invalid typed fields', () => {
    expect(
      scalarValueForField({ code: 'available', value_type: 'boolean' }, 'yes'),
    ).toBeUndefined();
    expect(
      scalarValueForField({ code: 'stock', value_type: 'integer' }, '12.5'),
    ).toBeUndefined();
  });
});
