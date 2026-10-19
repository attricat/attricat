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
