import { describe, expect, it } from 'vitest';
import i18n from '../../i18n';
import type { Attribute } from './api';
import { attributeHint, attributeMaxLength } from './attributeHints';

const t = i18n.t.bind(i18n);
const attribute = (overrides: Partial<Attribute>) =>
  ({ code: 'field', value_type: 'string', ...overrides }) as Attribute;

describe('attributeHint', () => {
  it('leads with the description and follows with text limits', () => {
    const sku = attribute({
      description: 'The code printed on the label.',
      value_schema: { type: 'string', minLength: 3, maxLength: 12 },
    });
    expect(attributeHint(sku, t)).toBe(
      'The code printed on the label. 3–12 characters.',
    );
    expect(attributeMaxLength(sku)).toBe(12);
    expect(
      attributeHint(attribute({ value_schema: { maxLength: 1 } }), t),
    ).toBe('Up to 1 character.');
  });

  it('describes number ranges and whole numbers', () => {
    expect(
      attributeHint(
        attribute({
          value_type: 'number',
          value_schema: { minimum: 0, maximum: 100 },
        }),
        t,
      ),
    ).toBe('A number from 0 to 100.');
    expect(attributeHint(attribute({ value_type: 'integer' }), t)).toBe(
      'A whole number.',
    );
    // A text limit means nothing for a number.
    expect(attributeMaxLength(attribute({ value_type: 'number' }))).toBe(
      undefined,
    );
  });

  it('describes exclusive bounds and steps', () => {
    const hint = (
      value_type: Attribute['value_type'],
      value_schema: Record<string, unknown>,
    ) => attributeHint(attribute({ value_type, value_schema }), t);
    expect(hint('number', { exclusiveMinimum: 0 })).toBe(
      'A number greater than 0.',
    );
    expect(hint('number', { exclusiveMinimum: 0, maximum: 1 })).toBe(
      'A number greater than 0 and at most 1.',
    );
    // The stricter of an inclusive and an exclusive bound applies.
    expect(hint('integer', { minimum: 5, exclusiveMinimum: 0 })).toBe(
      'A whole number of at least 5.',
    );
    expect(hint('integer', { minimum: 1, exclusiveMaximum: 10 })).toBe(
      'A whole number of at least 1 and less than 10.',
    );
    expect(hint('number', { minimum: 0, multipleOf: 0.5 })).toBe(
      'A number of at least 0. In steps of 0.5.',
    );
    expect(hint('integer', { multipleOf: 1 })).toBe('A whole number.');
  });

  it('describes a pattern by its hint and names known formats', () => {
    expect(
      attributeHint(
        attribute({
          value_schema: {
            maxLength: 8,
            pattern: '^[A-Z]{3}-[0-9]{4}$',
            'x-attricat-pattern-hint': 'Such as ABC-1234',
          },
        }),
        t,
      ),
    ).toBe('Up to 8 characters. Such as ABC-1234.');
    // A pattern without a hint is not shown as a regular expression.
    expect(
      attributeHint(attribute({ value_schema: { pattern: '^a$' } }), t),
    ).toBe(undefined);
    expect(
      attributeHint(attribute({ value_schema: { format: 'email' } }), t),
    ).toBe('An email address.');
  });

  it('summarizes a file policy', () => {
    expect(
      attributeHint(
        attribute({
          value_type: 'file',
          file_policy: {
            cardinality: 'many',
            ordered: true,
            allowed_mime_groups: ['image'],
            allowed_extensions: [],
            max_bytes: 5 * 1024 * 1024,
            purposes: [],
            image_only: true,
          },
        }),
        t,
      ),
    ).toBe('Images · up to 5.0 MB each · several files, in order.');
    expect(
      attributeHint(
        attribute({
          value_type: 'file',
          file_policy: {
            cardinality: 'one',
            ordered: false,
            allowed_mime_groups: [],
            allowed_extensions: ['.pdf'],
            purposes: [],
            image_only: false,
          },
        }),
        t,
      ),
    ).toBe('PDF · one file.');
  });

  it('says nothing without a description or limits', () => {
    expect(attributeHint(attribute({}), t)).toBe(undefined);
  });
});
