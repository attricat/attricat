import { describe, expect, it } from 'vitest';
import {
  hasInvalidScalarField,
  relationshipTargetsForForm,
  serializeAttributeValues,
  validateEntityForm,
  valuesForForm,
} from './entityForm';
import type { Attribute } from './api';
import { urlEditComponent } from '../views/components/urlComponents';

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
  it('validates only configured URL editors, including required and optional clearing', () => {
    const validate = (title: string, required: string[] = []) =>
      validateEntityForm(
        attributes,
        { title },
        required,
        undefined,
        undefined,
        new Map([['title', urlEditComponent.validateValue]]),
      );
    expect(validate('javascript:alert(1)').fieldErrors.title).toBeTruthy();
    expect(validate('https://example.com/a?q=1').fieldErrors).toEqual({});
    expect(validate('').fieldErrors).toEqual({});
    expect(validate('', ['title']).fieldErrors.title).toBeTruthy();
    expect(
      validateEntityForm(attributes, { title: 'ordinary text' }).fieldErrors,
    ).toEqual({});
    expect(
      serializeAttributeValues(attributes, {
        title: 'https://example.com/a?q=1',
      })[0],
    ).toMatchObject({ value: 'https://example.com/a?q=1' });
  });

  it('validates email value schemas while allowing optional empty values', () => {
    const attributes = [
      {
        code: 'email',
        value_type: 'string' as const,
        value_schema: { type: 'string', format: 'email' },
      },
    ];
    expect(
      validateEntityForm(attributes, { email: 'invalid' }).fieldErrors.email,
    ).toBeTruthy();
    expect(validateEntityForm(attributes, { email: '' }).fieldErrors).toEqual(
      {},
    );
    expect(
      validateEntityForm(attributes, { email: ' Name+tag@Example.com ' })
        .fieldErrors,
    ).toEqual({});
    expect(
      serializeAttributeValues(attributes, {
        email: ' Name+tag@Example.com ',
      })[0],
    ).toMatchObject({ value: 'Name+tag@Example.com' });
  });

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
    ).toEqual({
      fieldErrors: { price: 'Does not meet the schema requirements.' },
    });
  });

  it('maps required entity schema properties to their fields', () => {
    expect(
      validateEntityForm([{ code: 'title', value_type: 'string' }], {}, [], {
        type: 'object',
        required: ['title'],
      }),
    ).toEqual({
      fieldErrors: { title: 'Does not meet the schema requirements.' },
    });
  });
});
