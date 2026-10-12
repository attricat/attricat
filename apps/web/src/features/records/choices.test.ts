import { describe, expect, it } from 'vitest';
import '../../i18n';
import type { Attribute } from './api';
import { choiceLabel, choiceOptions } from './choices';

const attribute = (value_schema: unknown, value_type = 'string') =>
  ({ code: 'size', value_schema, value_type }) as Attribute;

describe('choiceOptions', () => {
  it('lists enum values with optional labels', () => {
    const options = choiceOptions(
      attribute({
        enum: ['s', 'l'],
        type: 'string',
        'x-attricat-enum-labels': { s: 'Small' },
      }),
    );
    expect(options).toEqual([
      { code: 's', label: 'Small' },
      { code: 'l', label: 'l' },
    ]);
    expect(choiceLabel(options!, 's')).toBe('Small');
    expect(choiceLabel(options!, 'x')).toBe('x');
  });

  it('resolves lexicon references in labels', () => {
    expect(
      choiceOptions(
        attribute({
          enum: ['s'],
          type: 'string',
          'x-attricat-enum-labels': { s: '{{Small}}' },
        }),
      ),
    ).toEqual([{ code: 's', label: 'Small' }]);
  });

  it('leaves other attributes alone', () => {
    for (const other of [
      attribute({ type: 'string' }),
      attribute({ enum: [1, 2], type: 'integer' }, 'integer'),
      attribute({ enum: ['a', 1] }),
      attribute({
        enum: ['draft'],
        type: 'string',
        'x-attricat-status': {
          options: [{ code: 'draft', label: 'Draft' }],
          version: 1,
        },
      }),
    ])
      expect(choiceOptions(other)).toBeUndefined();
  });
});
