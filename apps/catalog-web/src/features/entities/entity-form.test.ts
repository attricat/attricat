import { describe, expect, it } from 'vitest';
import {
  hasInvalidScalarField,
  relationshipTargetsForForm,
  serializeAttributeValues,
  validateEntityForm,
  valuesForForm,
} from './entity-form';
import type { Attribute } from './api';

const attributes = [
  { code: 'title', value_type: 'string' },
  { code: 'related_products', value_type: 'relationship' },
] satisfies Attribute[];
const entityIdOne = '123e4567-e89b-12d3-a456-426614174000';
const entityIdTwo = '123e4567-e89b-12d3-a456-426614174001';

const typedAttributes = [
  { code: 'price', value_type: 'number' },
  { code: 'stock', value_type: 'integer' },
  { code: 'available', value_type: 'boolean' },
  { code: 'cutoff', value_type: 'time' },
] as const satisfies readonly Attribute[];

describe('entity form values', () => {
  it('serializes scalar fields and complete relationship target sets', () => {
    expect(
      serializeAttributeValues(attributes, {
        title: 'Summer shirt',
        related_products: `${entityIdOne}, ${entityIdTwo} , `,
      }),
    ).toEqual([
      {
        kind: 'scalar',
        attribute_code: 'title',
        context_id: null,
        value: 'Summer shirt',
      },
    ]);
    expect(
      relationshipTargetsForForm(attributes, {
        related_products: `${entityIdOne}, ${entityIdTwo} , `,
      }),
    ).toEqual([
      {
        attribute_code: 'related_products',
        context_id: null,
        target_entity_ids: [entityIdOne, entityIdTwo],
      },
    ]);
  });

  it('rejects malformed relationship IDs', () => {
    expect(
      relationshipTargetsForForm(attributes, {
        related_products: 'not-a-uuid',
      }),
    ).toEqual([]);
  });

  it('reports malformed relationship IDs instead of allowing omission', () => {
    expect(
      validateEntityForm(attributes, {
        related_products: `${entityIdOne}, not-a-uuid`,
      }),
    ).toEqual({
      fieldErrors: {
        related_products: 'Enter comma-separated entity UUIDs.',
      },
    });
  });

  it('allows empty optional relationships and trims valid UUID lists', () => {
    expect(
      validateEntityForm(attributes, {
        related_products: ` ${entityIdOne}, ${entityIdTwo}, `,
      }),
    ).toEqual({ fieldErrors: {} });
    expect(validateEntityForm(attributes, {})).toEqual({ fieldErrors: {} });
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
      {
        kind: 'scalar',
        attribute_code: 'price',
        context_id: null,
        value: 49.95,
      },
      { kind: 'scalar', attribute_code: 'stock', context_id: null, value: 24 },
      {
        kind: 'scalar',
        attribute_code: 'available',
        context_id: null,
        value: false,
      },
      {
        kind: 'scalar',
        attribute_code: 'cutoff',
        context_id: null,
        value: { time: '09:30:00', time_zone: 'America/New_York' },
      },
    ]);
  });

  it('identifies typed values that fail their attribute schema', () => {
    expect(
      hasInvalidScalarField(
        [
          {
            code: 'price',
            value_type: 'number',
            value_schema: { type: 'number', minimum: 0 },
          },
        ],
        { price: '-1' },
      ),
    ).toBe(true);
  });

  it('maps entity schema property failures to their fields', () => {
    expect(
      validateEntityForm(
        [{ code: 'price', value_type: 'number' }],
        { price: '2' },
        [],
        {
          type: 'object',
          properties: { price: { minimum: 3 } },
        },
      ),
    ).toEqual({ fieldErrors: { price: 'must be >= 3' } });
  });

  it('maps required entity schema properties to their fields', () => {
    expect(
      validateEntityForm(
        [{ code: 'title', value_type: 'string' }],
        {},
        [],
        { type: 'object', required: ['title'] },
      ),
    ).toEqual({
      fieldErrors: { title: "must have required property 'title'" },
    });
  });
});
