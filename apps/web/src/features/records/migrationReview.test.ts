import { describe, expect, it } from 'vitest';
import type { Attribute } from './api';
import {
  answeredMigrationRelationships,
  migrationLayout,
  requiredMigrationAttributes,
} from './migrationReview';

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

describe('answeredMigrationRelationships', () => {
  const retargeted = [
    {
      attribute_code: 'brand',
      kind: 'relationship_target_changed',
      message: '',
    },
  ];

  it('leaves out a retargeted relationship left blank', () => {
    expect(
      answeredMigrationRelationships(
        [{ attribute_code: 'brand', target_record_ids: [] }],
        retargeted,
      ),
    ).toEqual([]);
  });

  it('keeps a replacement, and a relationship the user cleared', () => {
    const relationships = [
      { attribute_code: 'brand', target_record_ids: ['a'] },
      { attribute_code: 'owner', target_record_ids: [] },
    ];
    expect(answeredMigrationRelationships(relationships, retargeted)).toEqual(
      relationships,
    );
  });
});

describe('requiredMigrationAttributes', () => {
  const issues = [
    { attribute_code: 'sku', kind: 'missing_required', message: '' },
    {
      attribute_code: 'brand',
      kind: 'relationship_target_changed',
      message: '',
    },
  ];

  it('asks for a retargeted relationship until it is discarded', () => {
    expect(requiredMigrationAttributes(issues)).toEqual(['sku', 'brand']);
    expect(requiredMigrationAttributes(issues, ['brand'])).toEqual(['sku']);
  });
});
