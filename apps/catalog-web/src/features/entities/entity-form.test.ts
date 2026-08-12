import { describe, expect, it } from 'vitest';
import {
  relationshipTargetsForForm,
  serializeAttributeValues,
  valuesForForm,
} from './entity-form';

const attributes = [
  { code: 'title', value_type: 'string' },
  { code: 'related_products', value_type: 'relationship' },
];

const typedAttributes = [
  { code: 'price', value_type: 'number' },
  { code: 'stock', value_type: 'integer' },
  { code: 'available', value_type: 'boolean' },
  { code: 'cutoff', value_type: 'time' },
] as const;

describe('entity form values', () => {
  it('serializes scalar fields and complete relationship target sets', () => {
    expect(
      serializeAttributeValues(attributes, {
        title: 'Summer shirt',
        related_products: 'uuid-one, uuid-two , ',
      }),
    ).toEqual([
      { kind: 'scalar', attribute_code: 'title', value: 'Summer shirt' },
    ]);
    expect(
      relationshipTargetsForForm(attributes, {
        related_products: 'uuid-one, uuid-two , ',
      }),
    ).toEqual([
      {
        attribute_code: 'related_products',
        target_entity_ids: ['uuid-one', 'uuid-two'],
      },
    ]);
  });

  it('hydrates relationship values into a comma-separated field', () => {
    expect(
      valuesForForm(attributes, [
        { kind: 'scalar', attribute_code: 'title', value: 'Summer shirt' },
        {
          kind: 'relationship',
          attribute_code: 'related_products',
          target_entity_id: 'uuid-one',
        },
        {
          kind: 'relationship',
          attribute_code: 'related_products',
          target_entity_id: 'uuid-two',
        },
      ]),
    ).toEqual({
      title: 'Summer shirt',
      related_products: 'uuid-one, uuid-two',
    });
  });

  it('serializes typed scalar values without coercing false to an omission', () => {
    expect(
      serializeAttributeValues(typedAttributes, {
        price: '49.95',
        stock: '24',
        available: 'false',
        cutoff: '09:30:00 America/New_York',
      }),
    ).toEqual([
      { kind: 'scalar', attribute_code: 'price', value: 49.95 },
      { kind: 'scalar', attribute_code: 'stock', value: 24 },
      { kind: 'scalar', attribute_code: 'available', value: false },
      {
        kind: 'scalar',
        attribute_code: 'cutoff',
        value: { time: '09:30:00', time_zone: 'America/New_York' },
      },
    ]);
  });
});
