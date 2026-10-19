import { describe, expect, it } from 'vitest';
import type { Attribute } from './api';
import { migrationLayout } from './migrationReview';

const attributes = ['title', 'price', 'sku'].map(
  (code) => ({ code, value_type: 'string' }) as Attribute,
);
const labels = {
  review: 'Needs review',
  others: (count: number) => `Other fields (${count})`,
};

describe('migrationLayout', () => {
  it('puts fields to review first and collapses the rest', () => {
    expect(migrationLayout(attributes, new Set(['sku']), labels)).toEqual({
      type: 'stack',
      children: [
        { type: 'heading', text: 'Needs review' },
        { type: 'field', field: 'sku' },
        {
          type: 'accordion',
          sections: [
            {
              label: 'Other fields (2)',
              children: [
                { type: 'field', field: 'title' },
                { type: 'field', field: 'price' },
              ],
            },
          ],
        },
      ],
    });
  });

  it('shows every field when nothing needs review', () => {
    expect(migrationLayout(attributes, new Set(), labels)).toEqual({
      type: 'stack',
      children: attributes.map((attribute) => ({
        type: 'field',
        field: attribute.code,
      })),
    });
  });
});
