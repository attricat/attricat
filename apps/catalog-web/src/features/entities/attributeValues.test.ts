import { describe, expect, it } from 'vitest';
import { scalarValueForField, valueForField } from './attributeValues';

describe('attribute values', () => {
  it('preserves Markdown whitespace in strings while treating blank input as unset', () => {
    const attribute = { code: 'description', value_type: 'string' as const };
    const source = '    indented code\n\nline  \nnext\n';
    expect(scalarValueForField(attribute, source)?.value).toBe(source);
    expect(scalarValueForField(attribute, '  plain text  ')?.value).toBe(
      '  plain text  ',
    );
    expect(scalarValueForField(attribute, ' \n ')).toBeUndefined();
    expect(
      scalarValueForField({ code: 'count', value_type: 'integer' }, ' 12 ')
        ?.value,
    ).toBe(12);
  });
  it('formats API scalar values for form fields', () => {
    expect(valueForField(false)).toBe('false');
    expect(
      valueForField({ time: '09:30:00', time_zone: 'America/New_York' }),
    ).toBe('09:30:00 America/New_York');
  });

  it('parses fields into their declared scalar types', () => {
    expect(
      scalarValueForField(
        { code: 'available', value_type: 'boolean' },
        'false',
      ),
    ).toEqual({ kind: 'scalar', attribute_code: 'available', value: false });
    expect(
      scalarValueForField({ code: 'price', value_type: 'number' }, '49.95'),
    ).toEqual({ kind: 'scalar', attribute_code: 'price', value: 49.95 });
    expect(
      scalarValueForField(
        { code: 'cutoff', value_type: 'time' },
        '09:30 America/New_York',
      ),
    ).toEqual({
      kind: 'scalar',
      attribute_code: 'cutoff',
      value: { time: '09:30', time_zone: 'America/New_York' },
    });
  });

  it('round-trips JSON scalar fields without treating them as searchable text', () => {
    const attribute = { code: 'metadata', value_type: 'json' as const };
    expect(valueForField({ tags: ['new'] })).toBe('{"tags":["new"]}');
    expect(scalarValueForField(attribute, '{"tags":["new"]}')).toEqual({
      kind: 'scalar',
      attribute_code: 'metadata',
      value: { tags: ['new'] },
    });
    expect(scalarValueForField(attribute, '{not json}')).toBeUndefined();
  });

  it('does not silently coerce invalid typed fields', () => {
    expect(
      scalarValueForField({ code: 'available', value_type: 'boolean' }, 'yes'),
    ).toBeUndefined();
    expect(
      scalarValueForField({ code: 'stock', value_type: 'integer' }, '12.5'),
    ).toBeUndefined();
    expect(
      scalarValueForField(
        { code: 'release_date', value_type: 'date' },
        'tomorrow',
      ),
    ).toBeUndefined();
    expect(
      scalarValueForField(
        { code: 'cutoff', value_type: 'time' },
        '09:30 Not/A_Time_Zone',
      ),
    ).toBeUndefined();
  });

  it('enforces an attribute JSON Schema after type normalization', () => {
    const attribute = {
      code: 'price',
      value_type: 'number' as const,
      value_schema: { type: 'number', minimum: 0 },
    };
    expect(scalarValueForField(attribute, '49.95')).toEqual({
      kind: 'scalar',
      attribute_code: 'price',
      value: 49.95,
    });
    expect(scalarValueForField(attribute, '-1')).toBeUndefined();
  });
});
